/* Hosted compute runner for trusted SPIR-V. All passes share one storage
 * buffer at set 0, binding 0. Each pass selects a shader and supplies its own
 * grid and push constants. Dispatch order defines the dependencies.
 * --stream runs framed batches over one Vulkan session. Positional
 * shader paths run a single GPU1 batch, for stand-alone callers.
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>
#include <vulkan/vulkan.h>

enum { MAX_WORDS = 16 * 1024 * 1024, MAX_SHADERS = 16, MAX_PASSES = 64, PUSH_WORDS = 8 };

enum { MAX_PATH_BYTES = 4095, MAX_FAULT_BYTES = 1024, CACHED_PIPELINES = 2 * MAX_SHADERS };
enum { CACHED_READBACK_WORDS = 64 * 1024 };
_Static_assert(CACHED_PIPELINES <= 32, "pipeline cache usage must fit in a uint32_t");

struct adapter {
    VkPhysicalDevice device;
    uint32_t family;
};

static VkResult find_adapter(VkInstance instance, VkDeviceSize bytes,
                             const uint32_t groups[3], struct adapter *adapter) {
    uint32_t count = 0;
    VkResult result = vkEnumeratePhysicalDevices(instance, &count, NULL);
    if (result != VK_SUCCESS) return result;
    if (!count) return VK_ERROR_FEATURE_NOT_PRESENT;

    VkPhysicalDevice *devices = calloc(count, sizeof(*devices));
    if (!devices) return VK_ERROR_OUT_OF_HOST_MEMORY;
    result = vkEnumeratePhysicalDevices(instance, &count, devices);
    if (result != VK_SUCCESS && result != VK_INCOMPLETE) {
        free(devices);
        return result;
    }

    result = VK_ERROR_FEATURE_NOT_PRESENT;
    int best = 0;
    for (uint32_t i = 0; i < count; ++i) {
        VkPhysicalDeviceProperties properties;
        vkGetPhysicalDeviceProperties(devices[i], &properties);
        const VkPhysicalDeviceLimits *limits = &properties.limits;
        int preference;
        switch (properties.deviceType) {
            case VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU: preference = 4; break;
            case VK_PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU: preference = 3; break;
            case VK_PHYSICAL_DEVICE_TYPE_CPU: preference = 1; break;
            default: preference = 2; break;
        }
        if (preference <= best || bytes > limits->maxStorageBufferRange ||
            groups[0] > limits->maxComputeWorkGroupCount[0] ||
            groups[1] > limits->maxComputeWorkGroupCount[1] ||
            groups[2] > limits->maxComputeWorkGroupCount[2]) continue;

        uint32_t families = 0;
        vkGetPhysicalDeviceQueueFamilyProperties(devices[i], &families, NULL);
        if (!families) continue;
        VkQueueFamilyProperties *queues = calloc(families, sizeof(*queues));
        if (!queues) {
            free(devices);
            return VK_ERROR_OUT_OF_HOST_MEMORY;
        }
        vkGetPhysicalDeviceQueueFamilyProperties(devices[i], &families, queues);
        for (uint32_t q = 0; q < families; ++q) {
            if (queues[q].queueCount && (queues[q].queueFlags & VK_QUEUE_COMPUTE_BIT)) {
                *adapter = (struct adapter){devices[i], q};
                best = preference;
                result = VK_SUCCESS;
                break;
            }
        }
        free(queues);
    }
    free(devices);
    return result;
}

static VkResult visible_memory(VkPhysicalDevice device, uint32_t types,
                               int prefer_cached, uint32_t *index) {
    VkPhysicalDeviceMemoryProperties memory;
    vkGetPhysicalDeviceMemoryProperties(device, &memory);
    uint32_t fallback = UINT32_MAX;
    for (uint32_t i = 0; i < memory.memoryTypeCount; ++i) {
        const VkMemoryPropertyFlags flags = memory.memoryTypes[i].propertyFlags;
        if (!(types & (1u << i)) || !(flags & VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)) continue;
        if (!prefer_cached || (flags & VK_MEMORY_PROPERTY_HOST_CACHED_BIT)) {
            *index = i;
            return VK_SUCCESS;
        }
        if (fallback == UINT32_MAX) fallback = i;
    }
    if (fallback == UINT32_MAX) return VK_ERROR_FEATURE_NOT_PRESENT;
    *index = fallback;
    return VK_SUCCESS;
}

struct shader_code {
    uint32_t *words;
    size_t bytes;
};

struct pass {
    uint32_t shader;
    uint32_t groups[3];
    uint32_t push[PUSH_WORDS];
};

struct batch {
    uint32_t arena_words;
    uint32_t input_words;
    uint32_t result_at;
    uint32_t result_words;
    uint32_t pass_count;
    struct pass passes[MAX_PASSES];
    uint32_t shader_count;
    int export;
    struct shader_code shaders[MAX_SHADERS];
};

#define VK_TRY(call) do {                      \
    result = (call);                          \
    if (result != VK_SUCCESS) {               \
        *operation = #call;                   \
        goto cleanup;                         \
    }                                        \
} while (0)

struct cached_pipeline {
    struct shader_code code;
    VkPipeline pipeline;
};

struct gpu {
    VkDevice device;
    struct adapter adapter;
    VkPhysicalDeviceProperties properties;
    VkQueue queue;
    VkDescriptorSetLayout set_layout;
    VkDescriptorPool descriptors;
    VkDescriptorSet set;
    VkPipelineLayout layout;
    VkCommandPool commands;
    VkCommandBuffer command;
    VkFence fence;
    struct cached_pipeline shaders[CACHED_PIPELINES];
    uint32_t next_shader;
    int in_flight;
};

struct session {
    VkInstance instance;
    struct gpu gpu;
    int export;
};

static void close_gpu(struct gpu *gpu) {
    if (!gpu->device || gpu->in_flight) return;
    vkDestroyCommandPool(gpu->device, gpu->commands, NULL);
    vkDestroyFence(gpu->device, gpu->fence, NULL);
    for (uint32_t i = 0; i < CACHED_PIPELINES; ++i) {
        vkDestroyPipeline(gpu->device, gpu->shaders[i].pipeline, NULL);
        free(gpu->shaders[i].code.words);
    }
    vkDestroyPipelineLayout(gpu->device, gpu->layout, NULL);
    vkDestroyDescriptorPool(gpu->device, gpu->descriptors, NULL);
    vkDestroyDescriptorSetLayout(gpu->device, gpu->set_layout, NULL);
    vkDestroyDevice(gpu->device, NULL);
    *gpu = (struct gpu){0};
}

static VkResult use_adapter(struct session *live, VkDeviceSize bytes,
                            const uint32_t groups[3], int export, const char **operation) {
    VkResult result = VK_SUCCESS;
    if (live->instance && live->export != export) {
        close_gpu(&live->gpu);
        vkDestroyInstance(live->instance, NULL);
        live->instance = VK_NULL_HANDLE;
    }
    if (!live->instance) {
        const VkApplicationInfo app = {
            .sType = VK_STRUCTURE_TYPE_APPLICATION_INFO,
            .pApplicationName = "Console compute",
            .apiVersion = export ? VK_API_VERSION_1_1 : VK_API_VERSION_1_0,
        };
        const VkInstanceCreateInfo info = {
            .sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO, .pApplicationInfo = &app,
        };
        VK_TRY(vkCreateInstance(&info, NULL, &live->instance));
        live->export = export;
    }
    struct adapter adapter = {0};
    VK_TRY(find_adapter(live->instance, bytes, groups, &adapter));
    struct gpu *gpu = &live->gpu;
    if (gpu->adapter.device == adapter.device && gpu->adapter.family == adapter.family)
        return VK_SUCCESS;

    close_gpu(gpu);
    gpu->adapter = adapter;
    vkGetPhysicalDeviceProperties(adapter.device, &gpu->properties);
    const float priority = 1.0f;
    const VkDeviceQueueCreateInfo queue_info = {
        .sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
        .queueFamilyIndex = gpu->adapter.family, .queueCount = 1, .pQueuePriorities = &priority,
    };
    VkPhysicalDeviceFeatures available;
    vkGetPhysicalDeviceFeatures(gpu->adapter.device, &available);
    const VkPhysicalDeviceFeatures features = {.robustBufferAccess = available.robustBufferAccess};
    const char *extensions[] = {
        VK_KHR_EXTERNAL_MEMORY_FD_EXTENSION_NAME, VK_EXT_EXTERNAL_MEMORY_DMA_BUF_EXTENSION_NAME,
        VK_EXT_QUEUE_FAMILY_FOREIGN_EXTENSION_NAME,
    };
    const VkDeviceCreateInfo device_info = {
        .sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
        .queueCreateInfoCount = 1, .pQueueCreateInfos = &queue_info,
        .pEnabledFeatures = &features,
        .enabledExtensionCount = export ? sizeof(extensions) / sizeof(extensions[0]) : 0,
        .ppEnabledExtensionNames = extensions,
    };
    VK_TRY(vkCreateDevice(gpu->adapter.device, &device_info, NULL, &gpu->device));
    VkDevice device = gpu->device;
    vkGetDeviceQueue(device, gpu->adapter.family, 0, &gpu->queue);

    const VkDescriptorSetLayoutBinding binding = {
        .binding = 0, .descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
        .descriptorCount = 1, .stageFlags = VK_SHADER_STAGE_COMPUTE_BIT,
    };
    const VkDescriptorSetLayoutCreateInfo set_info = {
        .sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
        .bindingCount = 1, .pBindings = &binding,
    };
    VK_TRY(vkCreateDescriptorSetLayout(device, &set_info, NULL, &gpu->set_layout));
    const VkDescriptorPoolSize pool_size = {VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, 1};
    const VkDescriptorPoolCreateInfo pool_info = {
        .sType = VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO,
        .maxSets = 1, .poolSizeCount = 1, .pPoolSizes = &pool_size,
    };
    VK_TRY(vkCreateDescriptorPool(device, &pool_info, NULL, &gpu->descriptors));
    const VkDescriptorSetAllocateInfo set_allocate = {
        .sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO, .descriptorPool = gpu->descriptors,
        .descriptorSetCount = 1, .pSetLayouts = &gpu->set_layout,
    };
    VK_TRY(vkAllocateDescriptorSets(device, &set_allocate, &gpu->set));
    const VkPushConstantRange push = {.stageFlags = VK_SHADER_STAGE_COMPUTE_BIT,
                                     .size = PUSH_WORDS * sizeof(uint32_t)};
    const VkPipelineLayoutCreateInfo layout_info = {
        .sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO,
        .setLayoutCount = 1, .pSetLayouts = &gpu->set_layout,
        .pushConstantRangeCount = 1, .pPushConstantRanges = &push,
    };
    VK_TRY(vkCreatePipelineLayout(device, &layout_info, NULL, &gpu->layout));
    const VkCommandPoolCreateInfo commands_info = {
        .sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO, .queueFamilyIndex = gpu->adapter.family,
    };
    VK_TRY(vkCreateCommandPool(device, &commands_info, NULL, &gpu->commands));
    const VkCommandBufferAllocateInfo command_allocate = {
        .sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
        .commandPool = gpu->commands, .level = VK_COMMAND_BUFFER_LEVEL_PRIMARY, .commandBufferCount = 1,
    };
    VK_TRY(vkAllocateCommandBuffers(device, &command_allocate, &gpu->command));
    const VkFenceCreateInfo fence_info = {.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
    VK_TRY(vkCreateFence(device, &fence_info, NULL, &gpu->fence));

cleanup:
    return result;
}

static VkResult use_pipeline(struct gpu *gpu, struct shader_code *code,
                             uint32_t *used, VkPipeline *out, const char **operation) {
    for (uint32_t i = 0; i < CACHED_PIPELINES; ++i) {
        const struct cached_pipeline *entry = &gpu->shaders[i];
        if (entry->code.bytes == code->bytes &&
            memcmp(entry->code.words, code->words, code->bytes) == 0) {
            *used |= UINT32_C(1) << i;
            *out = entry->pipeline;
            return VK_SUCCESS;
        }
    }

    VkShaderModule shader = VK_NULL_HANDLE;
    VkPipeline pipeline = VK_NULL_HANDLE;
    VkResult result;
    const VkShaderModuleCreateInfo shader_info = {
        .sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
        .codeSize = code->bytes, .pCode = code->words,
    };
    VK_TRY(vkCreateShaderModule(gpu->device, &shader_info, NULL, &shader));
    const VkComputePipelineCreateInfo pipeline_info = {
        .sType = VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO,
        .stage = {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
            .stage = VK_SHADER_STAGE_COMPUTE_BIT, .module = shader, .pName = "main",
        },
        .layout = gpu->layout,
    };
    VK_TRY(vkCreateComputePipelines(gpu->device, VK_NULL_HANDLE, 1, &pipeline_info, NULL, &pipeline));

    // Keep pipelines already selected for this batch. A batch has at most 16 shaders.
    uint32_t i = gpu->next_shader;
    while (*used & (UINT32_C(1) << i)) i = (i + 1) % CACHED_PIPELINES;
    gpu->next_shader = (i + 1) % CACHED_PIPELINES;
    struct cached_pipeline *entry = &gpu->shaders[i];
    vkDestroyPipeline(gpu->device, entry->pipeline, NULL);
    free(entry->code.words);
    *entry = (struct cached_pipeline){*code, pipeline};
    *code = (struct shader_code){0};
    *used |= UINT32_C(1) << i;
    *out = pipeline;

cleanup:
    vkDestroyShaderModule(gpu->device, shader, NULL);
    if (result != VK_SUCCESS) vkDestroyPipeline(gpu->device, pipeline, NULL);
    return result;
}

static VkResult make_export(struct gpu *gpu, VkDeviceSize bytes,
                            VkBuffer *buffer, VkDeviceMemory *memory) {
    const VkExternalMemoryHandleTypeFlagBits dma = VK_EXTERNAL_MEMORY_HANDLE_TYPE_DMA_BUF_BIT_EXT;
    const VkPhysicalDeviceExternalBufferInfo query = {
        .sType = VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_EXTERNAL_BUFFER_INFO,
        .usage = VK_BUFFER_USAGE_TRANSFER_DST_BIT, .handleType = dma,
    };
    VkExternalBufferProperties sharing = {.sType = VK_STRUCTURE_TYPE_EXTERNAL_BUFFER_PROPERTIES};
    vkGetPhysicalDeviceExternalBufferProperties(gpu->adapter.device, &query, &sharing);
    if (!(sharing.externalMemoryProperties.externalMemoryFeatures & VK_EXTERNAL_MEMORY_FEATURE_EXPORTABLE_BIT))
        return VK_ERROR_FEATURE_NOT_PRESENT;

    const VkExternalMemoryBufferCreateInfo external = {
        .sType = VK_STRUCTURE_TYPE_EXTERNAL_MEMORY_BUFFER_CREATE_INFO, .handleTypes = dma,
    };
    const VkBufferCreateInfo info = {
        .sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO, .pNext = &external, .size = bytes,
        .usage = VK_BUFFER_USAGE_TRANSFER_DST_BIT, .sharingMode = VK_SHARING_MODE_EXCLUSIVE,
    };
    VkResult result = vkCreateBuffer(gpu->device, &info, NULL, buffer);
    if (result != VK_SUCCESS) return result;
    VkMemoryRequirements requirements;
    vkGetBufferMemoryRequirements(gpu->device, *buffer, &requirements);
    VkPhysicalDeviceMemoryProperties types;
    vkGetPhysicalDeviceMemoryProperties(gpu->adapter.device, &types);
    uint32_t chosen = UINT32_MAX;
    for (uint32_t i = 0; i < types.memoryTypeCount; ++i) {
        if (!(requirements.memoryTypeBits & (UINT32_C(1) << i))) continue;
        if (chosen == UINT32_MAX) chosen = i;
        if (types.memoryTypes[i].propertyFlags & VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT) {
            chosen = i;
            break;
        }
    }
    if (chosen == UINT32_MAX) return VK_ERROR_FEATURE_NOT_PRESENT;
    const VkMemoryDedicatedAllocateInfo dedicated = {
        .sType = VK_STRUCTURE_TYPE_MEMORY_DEDICATED_ALLOCATE_INFO, .buffer = *buffer,
    };
    const VkExportMemoryAllocateInfo exporting = {
        .sType = VK_STRUCTURE_TYPE_EXPORT_MEMORY_ALLOCATE_INFO, .pNext = &dedicated, .handleTypes = dma,
    };
    const VkMemoryAllocateInfo allocation = {
        .sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO, .pNext = &exporting,
        .allocationSize = requirements.size, .memoryTypeIndex = chosen,
    };
    result = vkAllocateMemory(gpu->device, &allocation, NULL, memory);
    if (result != VK_SUCCESS) return result;
    return vkBindBufferMemory(gpu->device, *buffer, *memory, 0);
}

static void copy_for_export(struct gpu *gpu, VkBuffer arena, VkDeviceSize offset,
                            VkBuffer shared, VkDeviceSize bytes) {
    const VkMemoryBarrier readable = {
        .sType = VK_STRUCTURE_TYPE_MEMORY_BARRIER,
        .srcAccessMask = VK_ACCESS_SHADER_WRITE_BIT, .dstAccessMask = VK_ACCESS_TRANSFER_READ_BIT,
    };
    vkCmdPipelineBarrier(gpu->command, VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT,
                         0, 1, &readable, 0, NULL, 0, NULL);
    const VkBufferCopy copy = {.srcOffset = offset, .size = bytes};
    vkCmdCopyBuffer(gpu->command, arena, shared, 1, &copy);
    const VkBufferMemoryBarrier release = {
        .sType = VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER, .srcAccessMask = VK_ACCESS_TRANSFER_WRITE_BIT,
        .srcQueueFamilyIndex = gpu->adapter.family, .dstQueueFamilyIndex = VK_QUEUE_FAMILY_FOREIGN_EXT,
        .buffer = shared, .size = VK_WHOLE_SIZE,
    };
    vkCmdPipelineBarrier(gpu->command, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT,
                         0, 0, NULL, 1, &release, 0, NULL);
}

static VkResult export_fd(VkDevice device, VkDeviceMemory memory, int *fd) {
    const PFN_vkGetMemoryFdKHR get_fd = (PFN_vkGetMemoryFdKHR)vkGetDeviceProcAddr(device, "vkGetMemoryFdKHR");
    if (!get_fd) return VK_ERROR_EXTENSION_NOT_PRESENT;
    const VkMemoryGetFdInfoKHR info = {
        .sType = VK_STRUCTURE_TYPE_MEMORY_GET_FD_INFO_KHR, .memory = memory,
        .handleType = VK_EXTERNAL_MEMORY_HANDLE_TYPE_DMA_BUF_BIT_EXT,
    };
    return get_fd(device, &info, fd);
}

static int send_frame(int fd) {
    char tag = 'D';
    struct iovec bytes = {.iov_base = &tag, .iov_len = sizeof(tag)};
    union { struct cmsghdr align; char bytes[CMSG_SPACE(sizeof(fd))]; } control = {0};
    struct msghdr message = {.msg_iov = &bytes, .msg_iovlen = 1,
                             .msg_control = control.bytes, .msg_controllen = sizeof(control.bytes)};
    struct cmsghdr *descriptor = CMSG_FIRSTHDR(&message);
    descriptor->cmsg_level = SOL_SOCKET;
    descriptor->cmsg_type = SCM_RIGHTS;
    descriptor->cmsg_len = CMSG_LEN(sizeof(fd));
    memcpy(CMSG_DATA(descriptor), &fd, sizeof(fd));
    return sendmsg(STDOUT_FILENO, &message, MSG_NOSIGNAL) == sizeof(tag);
}

static VkResult dispatch(struct session *live, struct batch *batch, uint32_t *words, int *fd,
                         const char **operation) {
    VkBuffer buffer = VK_NULL_HANDLE;
    VkDeviceMemory memory = VK_NULL_HANDLE;
    VkBuffer shared = VK_NULL_HANDLE;
    VkDeviceMemory sharing = VK_NULL_HANDLE;
    void *mapped = NULL;
    VkPipeline pipelines[MAX_SHADERS];
    VkResult result;

    const VkDeviceSize bytes = (VkDeviceSize)batch->arena_words * sizeof(uint32_t);
    uint32_t groups[3] = {0};
    for (uint32_t p = 0; p < batch->pass_count; ++p) {
        for (size_t axis = 0; axis < 3; ++axis) {
            if (batch->passes[p].groups[axis] > groups[axis]) groups[axis] = batch->passes[p].groups[axis];
        }
    }
    result = use_adapter(live, bytes, groups, batch->export, operation);
    if (result != VK_SUCCESS) return result;
    struct gpu *gpu = &live->gpu;
    VkDevice device = gpu->device;
    VkCommandBuffer command = gpu->command;

    VK_TRY(vkResetCommandPool(device, gpu->commands, 0));
    VK_TRY(vkResetFences(device, 1, &gpu->fence));
    uint32_t used = 0;
    for (uint32_t i = 0; i < batch->shader_count; ++i) {
        result = use_pipeline(gpu, &batch->shaders[i], &used, &pipelines[i], operation);
        if (result != VK_SUCCESS) goto cleanup;
    }

    const VkBufferCreateInfo buffer_info = {
        .sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO, .size = bytes,
        .usage = VK_BUFFER_USAGE_STORAGE_BUFFER_BIT | (batch->export ? VK_BUFFER_USAGE_TRANSFER_SRC_BIT : 0),
        .sharingMode = VK_SHARING_MODE_EXCLUSIVE,
    };
    VK_TRY(vkCreateBuffer(device, &buffer_info, NULL, &buffer));
    VkMemoryRequirements requirements;
    vkGetBufferMemoryRequirements(device, buffer, &requirements);
    uint32_t memory_type = 0;
    /* Readback-heavy jobs on integrated GPUs benefit from cacheable CPU loads. */
    const int prefer_cached = !batch->export &&
                              gpu->properties.deviceType == VK_PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU &&
                              (batch->result_words > batch->input_words ||
                               (batch->result_words == batch->input_words &&
                                batch->result_words >= CACHED_READBACK_WORDS));
    VK_TRY(visible_memory(gpu->adapter.device, requirements.memoryTypeBits, prefer_cached, &memory_type));
    const VkMemoryAllocateInfo allocation = {
        .sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
        .allocationSize = requirements.size, .memoryTypeIndex = memory_type,
    };
    VK_TRY(vkAllocateMemory(device, &allocation, NULL, &memory));
    VK_TRY(vkBindBufferMemory(device, buffer, memory, 0));
    VK_TRY(vkMapMemory(device, memory, 0, VK_WHOLE_SIZE, 0, &mapped));
    const size_t input_bytes = batch->input_words * sizeof(uint32_t);
    memcpy(mapped, words, input_bytes);
    memset((unsigned char *)mapped + input_bytes, 0, (size_t)bytes - input_bytes);
    const VkMappedMemoryRange mapping = {
        .sType = VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE, .memory = memory, .size = VK_WHOLE_SIZE,
    };
    VK_TRY(vkFlushMappedMemoryRanges(device, 1, &mapping));

    const VkDeviceSize output_bytes = (VkDeviceSize)batch->result_words * sizeof(uint32_t);
    if (batch->export) {
        VK_TRY(make_export(gpu, output_bytes, &shared, &sharing));
    }

    const VkDescriptorBufferInfo descriptor = {.buffer = buffer, .range = bytes};
    const VkWriteDescriptorSet write = {
        .sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET, .dstSet = gpu->set,
        .descriptorCount = 1, .descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
        .pBufferInfo = &descriptor,
    };
    vkUpdateDescriptorSets(device, 1, &write, 0, NULL);

    const VkCommandBufferBeginInfo begin = {
        .sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
        .flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT,
    };
    VK_TRY(vkBeginCommandBuffer(command, &begin));
    vkCmdBindDescriptorSets(command, VK_PIPELINE_BIND_POINT_COMPUTE, gpu->layout, 0, 1, &gpu->set, 0, NULL);
    const VkMemoryBarrier join = {
        .sType = VK_STRUCTURE_TYPE_MEMORY_BARRIER,
        .srcAccessMask = VK_ACCESS_SHADER_WRITE_BIT,
        .dstAccessMask = VK_ACCESS_SHADER_READ_BIT | VK_ACCESS_SHADER_WRITE_BIT,
    };
    for (uint32_t i = 0; i < batch->pass_count; ++i) {
        if (i != 0) {
            vkCmdPipelineBarrier(command, VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT,
                                 VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT, 0, 1, &join, 0, NULL, 0, NULL);
        }
        const struct pass *pass = &batch->passes[i];
        vkCmdBindPipeline(command, VK_PIPELINE_BIND_POINT_COMPUTE, pipelines[pass->shader]);
        vkCmdPushConstants(command, gpu->layout, VK_SHADER_STAGE_COMPUTE_BIT, 0, sizeof(pass->push), pass->push);
        vkCmdDispatch(command, pass->groups[0], pass->groups[1], pass->groups[2]);
    }

    if (batch->export) {
        copy_for_export(gpu, buffer, (VkDeviceSize)batch->result_at * sizeof(uint32_t), shared, output_bytes);
    } else {
        const VkMemoryBarrier readback = {
            .sType = VK_STRUCTURE_TYPE_MEMORY_BARRIER,
            .srcAccessMask = VK_ACCESS_SHADER_WRITE_BIT, .dstAccessMask = VK_ACCESS_HOST_READ_BIT,
        };
        vkCmdPipelineBarrier(command, VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT,
                             VK_PIPELINE_STAGE_HOST_BIT, 0, 1, &readback, 0, NULL, 0, NULL);
    }
    VK_TRY(vkEndCommandBuffer(command));
    const VkSubmitInfo submit = {
        .sType = VK_STRUCTURE_TYPE_SUBMIT_INFO, .commandBufferCount = 1, .pCommandBuffers = &command,
    };
    VK_TRY(vkQueueSubmit(gpu->queue, 1, &submit, gpu->fence));
    gpu->in_flight = 1;
    result = vkWaitForFences(device, 1, &gpu->fence, VK_TRUE, 30ull * 1000 * 1000 * 1000);
    gpu->in_flight = result != VK_SUCCESS && result != VK_ERROR_DEVICE_LOST;
    if (result != VK_SUCCESS) {
        *operation = "vkWaitForFences";
        goto cleanup;
    }
    if (batch->export) {
        VK_TRY(export_fd(device, sharing, fd));
    } else {
        VK_TRY(vkInvalidateMappedMemoryRanges(device, 1, &mapping));
        memcpy(words, (const uint32_t *)mapped + batch->result_at, output_bytes);
    }

cleanup:
    if (!gpu->in_flight) {
        if (mapped) vkUnmapMemory(device, memory);
        vkDestroyBuffer(device, buffer, NULL);
        vkFreeMemory(device, memory, NULL);
        vkDestroyBuffer(device, shared, NULL);
        vkFreeMemory(device, sharing, NULL);
    }
    return result;
}

static uint32_t little_endian(const unsigned char *byte) {
    return (uint32_t)byte[0] | ((uint32_t)byte[1] << 8) |
           ((uint32_t)byte[2] << 16) | ((uint32_t)byte[3] << 24);
}

static uint32_t *read_shader(const char *path, size_t *bytes) {
    FILE *file = fopen(path, "rb");
    if (!file) return NULL;
    uint32_t *code = NULL;
    if (fseek(file, 0, SEEK_END) != 0) goto done;
    const long length = ftell(file);
    if (length < 20 || length > 1024 * 1024 || length % sizeof(*code) != 0) goto done;
    if (fseek(file, 0, SEEK_SET) != 0) goto done;
    code = malloc((size_t)length);
    if (code && fread(code, 1, (size_t)length, file) != (size_t)length) {
        free(code);
        code = NULL;
    }
    if (code) {
        *bytes = (size_t)length;
        for (size_t i = 0; i < *bytes / sizeof(*code); ++i) {
            code[i] = little_endian((const unsigned char *)&code[i]);
        }
        if (code[0] != 0x07230203u) {
            free(code);
            code = NULL;
        }
    }
done:
    fclose(file);
    return code;
}

static int read_words(uint32_t *words, size_t count) {
    if (fread(words, sizeof(*words), count, stdin) != count) return 0;
    for (size_t i = 0; i < count; ++i) {
        words[i] = little_endian((const unsigned char *)&words[i]);
    }
    return 1;
}

static int read_plan(struct batch *batch) {
    uint32_t header[6];
    if (!read_words(header, 6) || (header[0] != 0x31555047u && header[0] != 0x32555047u)) return 0;
    batch->export = header[0] == 0x32555047u; /* GPU2 replies with a DMA-BUF descriptor. */
    batch->arena_words = header[1];
    batch->input_words = header[2];
    batch->result_at = header[3];
    batch->result_words = header[4];
    batch->pass_count = header[5];
    if (!batch->arena_words || batch->arena_words > MAX_WORDS ||
        batch->input_words > batch->arena_words || batch->result_at > batch->arena_words ||
        batch->result_words > batch->arena_words - batch->result_at ||
        (batch->export && !batch->result_words) ||
        !batch->pass_count || batch->pass_count > MAX_PASSES) return 0;

    for (uint32_t i = 0; i < batch->pass_count; ++i) {
        struct pass *pass = &batch->passes[i];
        if (!read_words(&pass->shader, 1) || pass->shader >= batch->shader_count ||
            !read_words(pass->groups, 3) || !pass->groups[0] || !pass->groups[1] || !pass->groups[2] ||
            !read_words(pass->push, PUSH_WORDS)) return 0;
    }
    return 1;
}

static int write_words(uint32_t *words, size_t count) {
    for (size_t i = 0; i < count; ++i) {
        const uint32_t word = words[i];
        unsigned char *byte = (unsigned char *)&words[i];
        for (size_t j = 0; j < sizeof(word); ++j) byte[j] = (unsigned char)(word >> (j * 8));
    }
    return fwrite(words, sizeof(*words), count, stdout) == count && fflush(stdout) == 0;
}

static int read_paths(struct batch *batch, const char *paths[MAX_SHADERS],
                      char storage[MAX_SHADERS][MAX_PATH_BYTES + 1]) {
    if (!read_words(&batch->shader_count, 1) ||
        !batch->shader_count || batch->shader_count > MAX_SHADERS) return 0;
    for (uint32_t i = 0; i < batch->shader_count; ++i) {
        uint32_t length;
        if (!read_words(&length, 1) || !length || length > MAX_PATH_BYTES ||
            fread(storage[i], 1, length, stdin) != length ||
            memchr(storage[i], '\0', length)) return 0;
        storage[i][length] = '\0';
        paths[i] = storage[i];
    }
    return 1;
}

static void report_fault(const char *fault, int stream) {
    if (!stream) {
        fprintf(stderr, "%s\n", fault);
        return;
    }
    uint32_t header[] = {UINT32_MAX, (uint32_t)strlen(fault)};
    if (write_words(header, 2)) {
        fwrite(fault, 1, strlen(fault), stdout);
        fflush(stdout);
    }
}

static int run_batch(struct session *live, struct batch *batch,
                     const char *paths[MAX_SHADERS], int stream) {
    uint32_t *words = NULL;
    int fd = -1;
    char fault[MAX_FAULT_BYTES] = {0};
    if (!read_plan(batch) || (batch->export && !stream)) {
        snprintf(fault, sizeof(fault), "Invalid compute plan");
        goto done;
    }

    const uint32_t host_words = batch->export || batch->input_words > batch->result_words
        ? batch->input_words : batch->result_words;
    words = malloc((host_words ? host_words : 1) * sizeof(*words));
    if (!words) {
        snprintf(fault, sizeof(fault), "Could not allocate compute input/output");
        goto done;
    }
    if (!read_words(words, batch->input_words) || (!stream && fgetc(stdin) != EOF) || ferror(stdin)) {
        snprintf(fault, sizeof(fault), "Input does not match the compute plan");
        goto done;
    }

    for (uint32_t i = 0; i < batch->shader_count; ++i) {
        struct shader_code *shader = &batch->shaders[i];
        shader->words = read_shader(paths[i], &shader->bytes);
        if (!shader->words) {
            snprintf(fault, sizeof(fault), "Could not load shader %s", paths[i]);
            goto done;
        }
    }

    const char *operation = NULL;
    const VkResult result = dispatch(live, batch, words, &fd, &operation);
    if (result != VK_SUCCESS) {
        snprintf(fault, sizeof(fault), "%s failed with Vulkan result %d", operation, result);
        goto done;
    }
    uint32_t count = batch->result_words;
    if (!stream) fprintf(stderr, "Vulkan adapter %s\n", live->gpu.properties.deviceName);
    const int wrote = batch->export ? send_frame(fd) :
        ((!stream || write_words(&count, 1)) && write_words(words, batch->result_words));
    if (!wrote) {
        snprintf(fault, sizeof(fault), "Could not write compute output");
    }

done:
    if (fd >= 0) close(fd);
    free(words);
    for (uint32_t i = 0; i < batch->shader_count; ++i) free(batch->shaders[i].words);
    if (!fault[0]) return 1;
    report_fault(fault, stream);
    return 0;
}

static int run_stream(struct session *live) {
    char storage[MAX_SHADERS][MAX_PATH_BYTES + 1];
    const char *paths[MAX_SHADERS];
    int next;
    while ((next = fgetc(stdin)) != EOF) {
        ungetc(next, stdin);
        struct batch batch = {0};
        if (!read_paths(&batch, paths, storage)) {
            report_fault("Invalid compute shader paths", 1);
            return 0;
        }
        if (!run_batch(live, &batch, paths, 1)) return 0;
    }
    return !ferror(stdin);
}

int main(int argc, char **argv) {
    struct session live = {0};
    int success;
    if (argc == 2 && strcmp(argv[1], "--stream") == 0) {
        success = run_stream(&live);
    } else if (argc >= 2 && argc - 1 <= MAX_SHADERS) {
        struct batch batch = {.shader_count = (uint32_t)(argc - 1)};
        const char *paths[MAX_SHADERS];
        for (uint32_t i = 0; i < batch.shader_count; ++i) paths[i] = argv[i + 1];
        success = run_batch(&live, &batch, paths, 0);
    } else {
        fprintf(stderr, "Usage: %s --stream | SHADER.spv [SHADER.spv ...]\n", argv[0]);
        return EXIT_FAILURE;
    }

    // A timed-out submission can still own Vulkan resources. Let the process exit.
    if (!live.gpu.in_flight) {
        close_gpu(&live.gpu);
        if (live.instance) vkDestroyInstance(live.instance, NULL);
    }
    return success ? EXIT_SUCCESS : EXIT_FAILURE;
}
