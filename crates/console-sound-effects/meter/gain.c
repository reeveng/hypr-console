#define _GNU_SOURCE
#include <endian.h>
#include <fcntl.h>
#include <limits.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

/* The LADSPA 1.1 binary interface. The observer owns no audio devices. */
typedef void *Handle;
struct Hint {
    int descriptor;
    float lower, upper;
};
typedef struct Descriptor {
    unsigned long id;
    const char *label;
    int properties;
    const char *name, *maker, *license;
    unsigned long port_count;
    const int *ports;
    const char *const *names;
    const struct Hint *hints;
    void *data;
    Handle (*instantiate)(const struct Descriptor *, unsigned long);
    void (*connect)(Handle, unsigned long, float *);
    void (*activate)(Handle);
    void (*run)(Handle, unsigned long);
    void (*run_adding)(Handle, unsigned long);
    void (*set_adding_gain)(Handle, float);
    void (*deactivate)(Handle);
    void (*cleanup)(Handle);
} Descriptor;

enum Port { GAIN, THROUGH, NODE_LOW, NODE_HIGH, PORTS };
enum PortFlags { INPUT = 1, OUTPUT = 2, CONTROL = 4, AUDIO = 8 };
enum Properties { HARD_RT_CAPABLE = 4 };
enum HintFlags { INTEGER_BOUNDED = 0x23 };

_Static_assert(ATOMIC_LLONG_LOCK_FREE == 2, "meter publication must be lock-free");
_Static_assert(sizeof(float) == sizeof(uint32_t), "speaker gain is binary32");
_Static_assert(sizeof(unsigned long long) == 8, "meter records are eight bytes");

struct Meter {
    float *port[PORTS];
    unsigned long interval, remaining;
    uint32_t sequence;
    _Atomic unsigned long long *published;
    char path[PATH_MAX];
};

static Handle instantiate(const Descriptor *descriptor, unsigned long rate)
{
    (void)descriptor;
    struct Meter *meter = calloc(1, sizeof(*meter));
    if (meter != NULL) {
        meter->interval = rate / 10 + 1;
    }
    return meter;
}

static void connect_port(Handle handle, unsigned long number, float *data)
{
    struct Meter *meter = handle;
    if (number < PORTS) {
        meter->port[number] = data;
    }
}

static void release(struct Meter *meter)
{
    if (meter->published != NULL) {
        munmap(meter->published, sizeof(*meter->published));
        unlink(meter->path);
        meter->published = NULL;
    }
}

static void activate(Handle handle)
{
    struct Meter *meter = handle;
    release(meter);
    const char *runtime = getenv("XDG_RUNTIME_DIR");
    const float *lo = meter->port[NODE_LOW], *hi = meter->port[NODE_HIGH];
    if (runtime == NULL || runtime[0] != '/' || lo == NULL || hi == NULL ||
        !(*lo >= 0 && *lo <= UINT16_MAX && *hi >= 0 && *hi <= UINT16_MAX)) {
        return;
    }
    uint32_t id = ((uint32_t)*hi << 16) | (uint32_t)*lo;
    int length = snprintf(meter->path, sizeof(meter->path),
                          "%s/console-playback-gain/meter-%u-XXXXXX", runtime, id);
    if (length < 0 || (size_t)length >= sizeof(meter->path)) {
        return;
    }
    int fd = mkostemp(meter->path, O_CLOEXEC);
    if (fd < 0) {
        return;
    }
    void *mapping = MAP_FAILED;
    if (ftruncate(fd, sizeof(*meter->published)) == 0) {
        mapping = mmap(NULL, sizeof(*meter->published), PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    }
    close(fd);
    if (mapping == MAP_FAILED) {
        unlink(meter->path);
        return;
    }
    meter->remaining = 0;
    meter->sequence = 0;
    meter->published = mapping;
    atomic_init(meter->published, 0);
}

/* The audio callback only passes through gain samples and publishes a scalar. */
static void run(Handle handle, unsigned long frames)
{
    struct Meter *meter = handle;
    const float *gain = meter->port[GAIN];
    float *through = meter->port[THROUGH];
    if (gain == NULL || frames == 0) {
        return;
    }
    if (through != NULL && through != gain) {
        memmove(through, gain, frames * sizeof(float));
    }
    if (meter->published == NULL) {
        return;
    }
    if (frames < meter->remaining) {
        meter->remaining -= frames;
        return;
    }
    meter->remaining = meter->interval;
    uint32_t bits;
    memcpy(&bits, &gain[frames - 1], sizeof(bits));
    uint64_t sample = ((uint64_t)++meter->sequence << 32) | bits;
    atomic_store_explicit(meter->published, htole64(sample), memory_order_relaxed);
}

static void cleanup(Handle handle)
{
    struct Meter *meter = handle;
    release(meter);
    free(meter);
}

static void deactivate(Handle handle)
{
    release(handle);
}

static const int ports[] = {INPUT | AUDIO, OUTPUT | AUDIO, INPUT | CONTROL, INPUT | CONTROL};
static const char *const names[] = {"Gain", "Through", "Node low", "Node high"};
static const struct Hint hints[] = {
    {0, 0, 0}, {0, 0, 0}, {INTEGER_BOUNDED, 0, UINT16_MAX}, {INTEGER_BOUNDED, 0, UINT16_MAX}
};

static const Descriptor descriptor = {
    .id = 767413, .label = "console_gain_meter", .properties = HARD_RT_CAPABLE,
    .name = "Console Playback Gain", .maker = "Console", .license = "AGPL-3.0-or-later",
    .port_count = PORTS, .ports = ports, .names = names, .hints = hints,
    .instantiate = instantiate, .connect = connect_port, .activate = activate,
    .run = run, .deactivate = deactivate, .cleanup = cleanup
};

__attribute__((visibility("default")))
const Descriptor *ladspa_descriptor(unsigned long index)
{
    return index == 0 ? &descriptor : NULL;
}
