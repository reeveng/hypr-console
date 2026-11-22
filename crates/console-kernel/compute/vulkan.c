/* Hosted Vulkan compute dispatch. Each workgroup has 64 lanes. One storage
 * buffer holds the input words, the checksums and optional reduction scratch.
 * Push constants identify each dispatch's input range and destination.
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <vulkan/vulkan.h>

enum { WORKGROUP_SIZE = 64, BLOCK_WORDS = 64, MAX_BLOCKS = 65536, MAX_LEVELS = 16 };

struct adapter {
    VkPhysicalDevice device;
    uint32_t family;
};

static VkResult find_adapter(VkInstance instance, VkDeviceSize bytes,
                             uint32_t groups, struct adapter *adapter) {
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
    for (uint32_t i = 0; i < count && !adapter->device; ++i) {
        VkPhysicalDeviceProperties properties;
        vkGetPhysicalDeviceProperties(devices[i], &properties);
        const VkPhysicalDeviceLimits *limits = &properties.limits;
        if (bytes > limits->maxStorageBufferRange ||
            groups > limits->maxComputeWorkGroupCount[0] ||
            WORKGROUP_SIZE > limits->maxComputeWorkGroupSize[0] ||
            WORKGROUP_SIZE > limits->maxComputeWorkGroupInvocations) continue;

        uint32_t families = 0;
        vkGetPhysicalDeviceQueueFamilyProperties(devices[i], &families, NULL);
        if (!families) continue;
        VkQueueFamilyProperties *queues = calloc(families, sizeof(*queues));
        if (!queues) {
            result = VK_ERROR_OUT_OF_HOST_MEMORY;
            break;
        }
        vkGetPhysicalDeviceQueueFamilyProperties(devices[i], &families, queues);
        for (uint32_t q = 0; q < families; ++q) {
            if (queues[q].queueCount && (queues[q].queueFlags & VK_QUEUE_COMPUTE_BIT)) {
                *adapter = (struct adapter){devices[i], q};
                result = VK_SUCCESS;
                break;
            }
        }
        free(queues);
    }
    free(devices);
    return result;
}

static VkResult visible_memory(VkPhysicalDevice device, uint32_t types, uint32_t *index) {
    VkPhysicalDeviceMemoryProperties memory;
    vkGetPhysicalDeviceMemoryProperties(device, &memory);
    for (uint32_t i = 0; i < memory.memoryTypeCount; ++i) {
        if ((types & (1u << i)) &&
            (memory.memoryTypes[i].propertyFlags & VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)) {
            *index = i;
            return VK_SUCCESS;
        }
    }
    return VK_ERROR_FEATURE_NOT_PRESENT;
}

struct dispatch {
    const uint32_t *shader;
    size_t shader_bytes;
    uint32_t *words;
    size_t input_words;
    uint32_t items;
    const struct reduction *reduction;
};

struct reduction {
    uint32_t widths[MAX_LEVELS];
    size_t levels;
};

struct stage {
    uint32_t inputs;
    uint32_t source;
    uint32_t destination;
    uint32_t reduce;
};

_Static_assert(sizeof(struct stage) == 4 * sizeof(uint32_t), "shader push constant layout");

static uint32_t parents(uint32_t inputs) {
    return (inputs + 1) / 2;
}

static uint32_t groups_for(uint32_t items) {
    return (items + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE;
}

static void record_stage(VkCommandBuffer command, VkPipelineLayout layout, struct stage stage) {
    const uint32_t outputs = stage.reduce ? parents(stage.inputs) : stage.inputs;
    vkCmdPushConstants(command, layout, VK_SHADER_STAGE_COMPUTE_BIT, 0, sizeof(stage), &stage);
    vkCmdDispatch(command, groups_for(outputs), 1, 1);
}

#define VK_TRY(call) do {                      \
    result = (call);                          \
    if (result != VK_SUCCESS) {               \
        *operation = #call;                   \
        goto cleanup;                         \
    }                                        \
} while (0)

static VkResult dispatch(struct dispatch batch, const char **operation,
                         char name[VK_MAX_PHYSICAL_DEVICE_NAME_SIZE]) {
    VkInstance instance = VK_NULL_HANDLE;
    VkDevice device = VK_NULL_HANDLE;
    VkBuffer buffer = VK_NULL_HANDLE;
    VkDeviceMemory memory = VK_NULL_HANDLE;
    VkDescriptorSetLayout set_layout = VK_NULL_HANDLE;
    VkDescriptorPool descriptors = VK_NULL_HANDLE;
    VkShaderModule shader = VK_NULL_HANDLE;
    VkPipelineLayout layout = VK_NULL_HANDLE;
    VkPipeline pipeline = VK_NULL_HANDLE;
    VkCommandPool commands = VK_NULL_HANDLE;
    VkFence fence = VK_NULL_HANDLE;
    void *mapped = NULL;
    int in_flight = 0;
    VkResult result;

    const uint32_t scratch = batch.reduction && batch.items > 1 ? parents(batch.items) : 0;
    const VkDeviceSize bytes = (batch.input_words + batch.items + scratch) * sizeof(uint32_t);
    const uint32_t groups = groups_for(batch.items);
    const VkApplicationInfo app = {
        .sType = VK_STRUCTURE_TYPE_APPLICATION_INFO,
        .pApplicationName = "Console compute",
        .apiVersion = VK_API_VERSION_1_0,
    };
    const VkInstanceCreateInfo instance_info = {
        .sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO, .pApplicationInfo = &app,
    };
    VK_TRY(vkCreateInstance(&instance_info, NULL, &instance));

    struct adapter adapter = {0};
    VK_TRY(find_adapter(instance, bytes, groups, &adapter));
    VkPhysicalDeviceProperties properties;
    vkGetPhysicalDeviceProperties(adapter.device, &properties);
    memcpy(name, properties.deviceName, VK_MAX_PHYSICAL_DEVICE_NAME_SIZE);

    const float priority = 1.0f;
    const VkDeviceQueueCreateInfo queue_info = {
        .sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
        .queueFamilyIndex = adapter.family, .queueCount = 1, .pQueuePriorities = &priority,
    };
    const VkDeviceCreateInfo device_info = {
        .sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
        .queueCreateInfoCount = 1, .pQueueCreateInfos = &queue_info,
    };
    VK_TRY(vkCreateDevice(adapter.device, &device_info, NULL, &device));
    VkQueue queue;
    vkGetDeviceQueue(device, adapter.family, 0, &queue);

    const VkBufferCreateInfo buffer_info = {
        .sType = VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO, .size = bytes,
        .usage = VK_BUFFER_USAGE_STORAGE_BUFFER_BIT, .sharingMode = VK_SHARING_MODE_EXCLUSIVE,
    };
    VK_TRY(vkCreateBuffer(device, &buffer_info, NULL, &buffer));
    VkMemoryRequirements requirements;
    vkGetBufferMemoryRequirements(device, buffer, &requirements);
    uint32_t memory_type = 0;
    VK_TRY(visible_memory(adapter.device, requirements.memoryTypeBits, &memory_type));
    const VkMemoryAllocateInfo allocation = {
        .sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
        .allocationSize = requirements.size, .memoryTypeIndex = memory_type,
    };
    VK_TRY(vkAllocateMemory(device, &allocation, NULL, &memory));
    VK_TRY(vkBindBufferMemory(device, buffer, memory, 0));
    VK_TRY(vkMapMemory(device, memory, 0, VK_WHOLE_SIZE, 0, &mapped));
    memcpy(mapped, batch.words, batch.input_words * sizeof(uint32_t));
    const VkMappedMemoryRange mapping = {
        .sType = VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE, .memory = memory, .size = VK_WHOLE_SIZE,
    };
    VK_TRY(vkFlushMappedMemoryRanges(device, 1, &mapping));

    const VkDescriptorSetLayoutBinding binding = {
        .binding = 0, .descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
        .descriptorCount = 1, .stageFlags = VK_SHADER_STAGE_COMPUTE_BIT,
    };
    const VkDescriptorSetLayoutCreateInfo set_info = {
        .sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
        .bindingCount = 1, .pBindings = &binding,
    };
    VK_TRY(vkCreateDescriptorSetLayout(device, &set_info, NULL, &set_layout));
    const VkDescriptorPoolSize pool_size = {VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, 1};
    const VkDescriptorPoolCreateInfo pool_info = {
        .sType = VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO,
        .maxSets = 1, .poolSizeCount = 1, .pPoolSizes = &pool_size,
    };
    VK_TRY(vkCreateDescriptorPool(device, &pool_info, NULL, &descriptors));
    const VkDescriptorSetAllocateInfo set_allocate = {
        .sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO, .descriptorPool = descriptors,
        .descriptorSetCount = 1, .pSetLayouts = &set_layout,
    };
    VkDescriptorSet set;
    VK_TRY(vkAllocateDescriptorSets(device, &set_allocate, &set));
    const VkDescriptorBufferInfo descriptor = {.buffer = buffer, .range = bytes};
    const VkWriteDescriptorSet write = {
        .sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET, .dstSet = set,
        .descriptorCount = 1, .descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
        .pBufferInfo = &descriptor,
    };
    vkUpdateDescriptorSets(device, 1, &write, 0, NULL);

    const VkShaderModuleCreateInfo shader_info = {
        .sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
        .codeSize = batch.shader_bytes, .pCode = batch.shader,
    };
    VK_TRY(vkCreateShaderModule(device, &shader_info, NULL, &shader));
    const VkPushConstantRange push = {.stageFlags = VK_SHADER_STAGE_COMPUTE_BIT,
                                     .size = sizeof(struct stage)};
    const VkPipelineLayoutCreateInfo layout_info = {
        .sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO,
        .setLayoutCount = 1, .pSetLayouts = &set_layout,
        .pushConstantRangeCount = 1, .pPushConstantRanges = &push,
    };
    VK_TRY(vkCreatePipelineLayout(device, &layout_info, NULL, &layout));
    const VkComputePipelineCreateInfo pipeline_info = {
        .sType = VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO,
        .stage = {
            .sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO,
            .stage = VK_SHADER_STAGE_COMPUTE_BIT, .module = shader, .pName = "main",
        },
        .layout = layout,
    };
    VK_TRY(vkCreateComputePipelines(device, VK_NULL_HANDLE, 1, &pipeline_info, NULL, &pipeline));

    const VkCommandPoolCreateInfo commands_info = {
        .sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO, .queueFamilyIndex = adapter.family,
    };
    VK_TRY(vkCreateCommandPool(device, &commands_info, NULL, &commands));
    const VkCommandBufferAllocateInfo command_allocate = {
        .sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
        .commandPool = commands, .level = VK_COMMAND_BUFFER_LEVEL_PRIMARY, .commandBufferCount = 1,
    };
    VkCommandBuffer command;
    VK_TRY(vkAllocateCommandBuffers(device, &command_allocate, &command));
    const VkCommandBufferBeginInfo begin = {
        .sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
        .flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT,
    };
    VK_TRY(vkBeginCommandBuffer(command, &begin));
    vkCmdBindPipeline(command, VK_PIPELINE_BIND_POINT_COMPUTE, pipeline);
    vkCmdBindDescriptorSets(command, VK_PIPELINE_BIND_POINT_COMPUTE, layout, 0, 1, &set, 0, NULL);
    uint32_t result_at = (uint32_t)batch.input_words;
    record_stage(command, layout, (struct stage){
        .inputs = batch.items, .destination = result_at,
    });

    if (batch.reduction) {
        uint32_t other = result_at + batch.items;
        const VkMemoryBarrier join = {
            .sType = VK_STRUCTURE_TYPE_MEMORY_BARRIER,
            .srcAccessMask = VK_ACCESS_SHADER_WRITE_BIT,
            .dstAccessMask = VK_ACCESS_SHADER_READ_BIT | VK_ACCESS_SHADER_WRITE_BIT,
        };
        for (size_t i = 0; i < batch.reduction->levels; ++i) {
            vkCmdPipelineBarrier(command, VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT,
                                 VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT, 0, 1, &join, 0, NULL, 0, NULL);
            record_stage(command, layout, (struct stage){
                .inputs = batch.reduction->widths[i], .source = result_at,
                .destination = other, .reduce = 1,
            });
            const uint32_t previous = result_at;
            result_at = other;
            other = previous;
        }
    }

    const VkMemoryBarrier readback = {
        .sType = VK_STRUCTURE_TYPE_MEMORY_BARRIER,
        .srcAccessMask = VK_ACCESS_SHADER_WRITE_BIT, .dstAccessMask = VK_ACCESS_HOST_READ_BIT,
    };
    vkCmdPipelineBarrier(command, VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT,
                         VK_PIPELINE_STAGE_HOST_BIT, 0, 1, &readback, 0, NULL, 0, NULL);
    VK_TRY(vkEndCommandBuffer(command));
    const VkFenceCreateInfo fence_info = {.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO};
    VK_TRY(vkCreateFence(device, &fence_info, NULL, &fence));
    const VkSubmitInfo submit = {
        .sType = VK_STRUCTURE_TYPE_SUBMIT_INFO, .commandBufferCount = 1, .pCommandBuffers = &command,
    };
    VK_TRY(vkQueueSubmit(queue, 1, &submit, fence));
    in_flight = 1;
    VK_TRY(vkWaitForFences(device, 1, &fence, VK_TRUE, UINT64_MAX));
    in_flight = 0;
    VK_TRY(vkInvalidateMappedMemoryRanges(device, 1, &mapping));
    const uint32_t results = batch.reduction ? 1 : batch.items;
    memcpy(batch.words + batch.input_words, (const uint32_t *)mapped + result_at,
           results * sizeof(uint32_t));

cleanup:
    if (device) {
        if (in_flight) {
            const VkResult idle = vkDeviceWaitIdle(device);
            if (idle != VK_SUCCESS && idle != VK_ERROR_DEVICE_LOST) return result;
        }
        vkDestroyFence(device, fence, NULL);
        vkDestroyCommandPool(device, commands, NULL);
        vkDestroyPipeline(device, pipeline, NULL);
        vkDestroyPipelineLayout(device, layout, NULL);
        vkDestroyShaderModule(device, shader, NULL);
        vkDestroyDescriptorPool(device, descriptors, NULL);
        vkDestroyDescriptorSetLayout(device, set_layout, NULL);
        if (mapped) vkUnmapMemory(device, memory);
        vkDestroyBuffer(device, buffer, NULL);
        vkFreeMemory(device, memory, NULL);
        vkDestroyDevice(device, NULL);
    }
    if (instance) vkDestroyInstance(instance, NULL);
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

static int read_count(const char *text, uint32_t *count) {
    char *tail;
    const unsigned long value = strtoul(text, &tail, 10);
    if (text[0] < '0' || text[0] > '9' || *tail || value > MAX_BLOCKS) return 0;
    *count = (uint32_t)value;
    return 1;
}

/* The job producer supplies the widths. Check that it supplied exactly the
 * shrinking levels before using any width to record device commands.
 */
static int read_reduction(char **arguments, int length, uint32_t inputs, struct reduction *plan) {
    if (length > MAX_LEVELS) return 0;
    for (int i = 0; i < length; ++i) {
        uint32_t width;
        if (inputs <= 1 || !read_count(arguments[i], &width) || width != inputs) return 0;
        plan->widths[plan->levels++] = width;
        inputs = parents(inputs);
    }
    return inputs <= 1;
}

int main(int argc, char **argv) {
    if (argc < 3 || (argc > 3 && strcmp(argv[3], "--reduce") != 0)) {
        fprintf(stderr, "Usage: %s SHADER.spv BLOCK_COUNT [--reduce [LEVEL_WIDTH ...]]\n", argv[0]);
        return EXIT_FAILURE;
    }
    uint32_t count;
    if (!read_count(argv[2], &count)) {
        fputs("Invalid checksum batch size\n", stderr);
        return EXIT_FAILURE;
    }
    struct reduction plan = {0};
    const struct reduction *reduction = NULL;
    if (argc > 3) {
        if (!read_reduction(argv + 4, argc - 4, count, &plan)) {
            fputs("Invalid checksum reduction plan\n", stderr);
            return EXIT_FAILURE;
        }
        reduction = &plan;
    }
    if (count == 0) {
        if (fgetc(stdin) != EOF || ferror(stdin)) return EXIT_FAILURE;
        const unsigned char zero[4] = {0};
        if (reduction && (fwrite(zero, 1, sizeof(zero), stdout) != sizeof(zero) || fflush(stdout) != 0)) {
            return EXIT_FAILURE;
        }
        return EXIT_SUCCESS;
    }

    int status = EXIT_FAILURE;
    size_t shader_bytes = 0;
    uint32_t *shader = read_shader(argv[1], &shader_bytes);
    const size_t input_words = count * BLOCK_WORDS;
    uint32_t *words = malloc((input_words + count) * sizeof(*words));
    if (!shader || !words) {
        fputs("Could not load the shader or allocate the checksum batch\n", stderr);
        goto done;
    }
    if (fread(words, sizeof(*words), input_words, stdin) != input_words ||
        fgetc(stdin) != EOF || ferror(stdin)) {
        fputs("Input does not match the checksum batch size\n", stderr);
        goto done;
    }
    for (size_t i = 0; i < input_words; ++i) {
        words[i] = little_endian((const unsigned char *)&words[i]);
    }

    const char *operation = NULL;
    char name[VK_MAX_PHYSICAL_DEVICE_NAME_SIZE] = {0};
    const VkResult result = dispatch((struct dispatch){shader, shader_bytes, words, input_words,
                                                       count, reduction}, &operation, name);
    if (result != VK_SUCCESS) {
        fprintf(stderr, "%s failed with Vulkan result %d\n", operation, result);
        goto done;
    }
    fprintf(stderr, "Vulkan adapter %s\n", name);
    const uint32_t results = reduction ? 1 : count;
    uint32_t *output = words + input_words;
    for (size_t i = 0; i < results; ++i) {
        const uint32_t hash = output[i];
        unsigned char *byte = (unsigned char *)&output[i];
        for (size_t j = 0; j < sizeof(hash); ++j) byte[j] = (unsigned char)(hash >> (j * 8));
    }
    if (fwrite(output, sizeof(*output), results, stdout) != results || fflush(stdout) != 0) {
        fputs("Could not write checksums\n", stderr);
        goto done;
    }
    status = EXIT_SUCCESS;
done:
    free(words);
    free(shader);
    return status;
}
