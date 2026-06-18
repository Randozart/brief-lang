// Brief GPU Runtime — Vulkan Compute Backend
//
// Provides a portable GPU dispatch layer for Brief's #gpu / #?gpu offloading.
// Uses Vulkan compute for maximum hardware portability (NVIDIA, AMD, Intel,
// Apple via MoltenVK, software via LLVMPipe/Mesa).
//
// Falls back gracefully to CPU when Vulkan is unavailable — binaries compiled
// with --gpu-offload work on any machine.

#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <dlfcn.h>

// ---------------------------------------------------------------------------
// Optional Vulkan function pointer table
// ---------------------------------------------------------------------------
// We load libvulkan.so.1 dynamically. If it's not present, all dispatch
// functions return 0/false, and the CPU fallback path is used.

#define VK_NULL_HANDLE 0
#define VK_SUCCESS 0

typedef uint64_t VkInstance_T;
typedef uint64_t VkDevice_T;
typedef uint64_t VkBuffer_T;
typedef uint64_t VkDeviceMemory_T;
typedef uint64_t VkShaderModule_T;
typedef uint64_t VkPipeline_T;
typedef uint64_t VkPipelineLayout_T;
typedef uint64_t VkDescriptorSetLayout_T;
typedef uint64_t VkDescriptorPool_T;
typedef uint64_t VkDescriptorSet_T;
typedef uint64_t VkCommandPool_T;
typedef uint64_t VkCommandBuffer_T;
typedef uint64_t VkFence_T;

typedef VkInstance_T* VkInstance;
typedef VkDevice_T* VkDevice;
typedef VkBuffer_T* VkBuffer;
typedef VkDeviceMemory_T* VkDeviceMemory;
typedef VkShaderModule_T* VkShaderModule;
typedef VkPipeline_T* VkPipeline;
typedef VkPipelineLayout_T* VkPipelineLayout;
typedef VkDescriptorSetLayout_T* VkDescriptorSetLayout;
typedef VkDescriptorPool_T* VkDescriptorPool;
typedef VkDescriptorSet_T* VkDescriptorSet;
typedef VkCommandPool_T* VkCommandPool;
typedef VkCommandBuffer_T* VkCommandBuffer;
typedef VkFence_T* VkFence;

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

static void* vk_lib = NULL;
static int vk_initialized = 0;
static int vk_available = 0;

// OpenCL type declarations (needed before state variables)
typedef void* cl_platform_id;
typedef void* cl_device_id;
typedef void* cl_context;
typedef void* cl_command_queue;
typedef void* cl_program;
typedef void* cl_kernel;
typedef void* cl_mem;
typedef struct { void* data; } cl_event;

// OpenCL state — declared early so brief_gpu_is_available can check both backends
static void* cl_lib = NULL;
static int cl_available = 0;
static cl_platform_id cl_platform;
static cl_device_id cl_device;
static cl_context cl_ctx;
static cl_command_queue cl_queue;

// OpenCL function pointer declarations (used by dispatch functions below)
static cl_platform_id* (*clGetPlatformIDs)(uint32_t, cl_platform_id*, uint32_t*) = NULL;
static int (*clGetDeviceIDs)(cl_platform_id, uint64_t, uint32_t, cl_device_id*, uint32_t*) = NULL;
static cl_context (*clCreateContext)(void*, uint32_t, cl_device_id*, void*, void*, int*) = NULL;
static cl_command_queue (*clCreateCommandQueue)(cl_context, cl_device_id, uint64_t, int*) = NULL;
static cl_program (*clCreateProgramWithIL)(cl_context, const void*, size_t, int*) = NULL;
static int (*clBuildProgram)(cl_program, uint32_t, const cl_device_id*, const char*, void*, void*) = NULL;
static cl_kernel (*clCreateKernel)(cl_program, const char*, int*) = NULL;
static int (*clSetKernelArgSVMPointer)(cl_kernel, uint32_t, void*) = NULL;
static int (*clSetKernelArg)(cl_kernel, uint32_t, size_t, const void*) = NULL;
static int (*clEnqueueNDRangeKernel)(cl_command_queue, cl_kernel, uint32_t, const size_t*, const size_t*, const size_t*, uint32_t, const cl_event*, cl_event*) = NULL;
static int (*clEnqueueReadBuffer)(cl_command_queue, cl_mem, int, size_t, size_t, void*, uint32_t, const cl_event*, cl_event*) = NULL;
static int (*clEnqueueWriteBuffer)(cl_command_queue, cl_mem, int, size_t, size_t, const void*, uint32_t, const cl_event*, cl_event*) = NULL;
static int (*clFinish)(cl_command_queue) = NULL;
static int (*clReleaseKernel)(cl_kernel) = NULL;
static int (*clReleaseProgram)(cl_program) = NULL;
static int (*clReleaseCommandQueue)(cl_command_queue) = NULL;
static int (*clReleaseContext)(cl_context) = NULL;
static int (*clReleaseMemObject)(cl_mem) = NULL;
static void* (*clSVMAlloc)(cl_context, uint64_t, size_t, uint32_t) = NULL;
static int (*clSVMFree)(cl_context, void*) = NULL;
static cl_mem (*clCreateBuffer)(cl_context, uint64_t, size_t, void*, int*) = NULL;
static int (*clGetProgramBuildInfo)(cl_program, cl_device_id, uint32_t, size_t, void*, size_t*) = NULL;

static VkInstance vk_instance;
static VkDevice vk_device;
static VkPipelineLayout vk_pipeline_layout;
static VkDescriptorSetLayout vk_desc_set_layout;
static VkDescriptorPool vk_desc_pool;
static VkCommandPool vk_cmd_pool;
static VkCommandBuffer vk_cmd_buf;
static VkFence vk_fence;

static uint32_t vk_queue_family_index = UINT32_MAX;
static uint64_t vk_queue = 0;

// Buffer tracking for device memory handles
#define MAX_GPU_BUFFERS 64
static struct {
    int used;
    void* host_ptr;
    VkDeviceMemory memory;
    VkBuffer buffer;
    size_t size;
} gpu_buffers[MAX_GPU_BUFFERS];
static int next_buffer_id = 1;  // 0 = invalid

// ---------------------------------------------------------------------------
// Vulkan function pointer declarations (loaded via dlsym)
// ---------------------------------------------------------------------------

static int (*vkCreateInstance)(const void*, const void*, VkInstance*) = NULL;
static void (*vkDestroyInstance)(VkInstance, const void*) = NULL;
static int (*vkEnumeratePhysicalDevices)(VkInstance, uint32_t*, void*) = NULL;
static void (*vkGetPhysicalDeviceProperties)(void*, void*) = NULL;
static void (*vkGetPhysicalDeviceQueueFamilyProperties)(void*, uint32_t*, void*) = NULL;
static int (*vkCreateDevice)(void*, const void*, const void*, VkDevice*) = NULL;
static void (*vkGetDeviceQueue)(VkDevice, uint32_t, uint32_t, uint64_t*) = NULL;
static void (*vkDestroyDevice)(VkDevice, const void*) = NULL;
static int (*vkCreateBuffer)(VkDevice, const void*, const void*, VkBuffer*) = NULL;
static int (*vkGetBufferMemoryRequirements)(VkDevice, VkBuffer, void*) = NULL;
static int (*vkAllocateMemory)(VkDevice, const void*, const void*, VkDeviceMemory*) = NULL;
static int (*vkBindBufferMemory)(VkDevice, VkBuffer, VkDeviceMemory, uint64_t) = NULL;
static void* (*vkMapMemory)(VkDevice, VkDeviceMemory, uint64_t, uint64_t, uint32_t) = NULL;
static void (*vkUnmapMemory)(VkDevice, VkDeviceMemory) = NULL;
static void (*vkFreeMemory)(VkDevice, VkDeviceMemory, const void*) = NULL;
static void (*vkDestroyBuffer)(VkDevice, VkBuffer, const void*) = NULL;
static int (*vkCreateShaderModule)(VkDevice, const void*, const void*, VkShaderModule*) = NULL;
static void (*vkDestroyShaderModule)(VkDevice, VkShaderModule, const void*) = NULL;
static int (*vkCreateDescriptorSetLayout)(VkDevice, const void*, const void*, VkDescriptorSetLayout*) = NULL;
static int (*vkCreatePipelineLayout)(VkDevice, const void*, const void*, VkPipelineLayout*) = NULL;
static int (*vkCreateComputePipelines)(VkDevice, uint64_t, uint32_t, const void*, const void*, VkPipeline*) = NULL;
static void (*vkDestroyPipeline)(VkDevice, VkPipeline, const void*) = NULL;
static void (*vkDestroyPipelineLayout)(VkDevice, VkPipelineLayout, const void*) = NULL;
static void (*vkDestroyDescriptorSetLayout)(VkDevice, VkDescriptorSetLayout, const void*) = NULL;
static int (*vkCreateDescriptorPool)(VkDevice, const void*, const void*, VkDescriptorPool*) = NULL;
static int (*vkAllocateDescriptorSets)(VkDevice, const void*, VkDescriptorSet*) = NULL;
static void (*vkUpdateDescriptorSets)(VkDevice, uint32_t, const void*, uint32_t, const void*) = NULL;
static int (*vkCreateCommandPool)(VkDevice, const void*, const void*, VkCommandPool*) = NULL;
static int (*vkAllocateCommandBuffers)(VkDevice, const void*, VkCommandBuffer*) = NULL;
static int (*vkBeginCommandBuffer)(VkCommandBuffer, const void*) = NULL;
static int (*vkCmdBindPipeline)(VkCommandBuffer, uint32_t, VkPipeline) = NULL;
static int (*vkCmdBindDescriptorSets)(VkCommandBuffer, uint32_t, VkPipelineLayout, uint32_t, uint32_t, const VkDescriptorSet*, uint32_t, const uint32_t*) = NULL;
static int (*vkCmdDispatch)(VkCommandBuffer, uint32_t, uint32_t, uint32_t) = NULL;
static int (*vkEndCommandBuffer)(VkCommandBuffer) = NULL;
static int (*vkQueueSubmit)(uint64_t, uint32_t, const void*, VkFence) = NULL;
static int (*vkWaitForFences)(VkDevice, uint32_t, const VkFence*, uint32_t, uint64_t) = NULL;
static int (*vkCreateFence)(VkDevice, const void*, const void*, VkFence*) = NULL;
static void (*vkDestroyFence)(VkDevice, VkFence, const void*) = NULL;
static void (*vkDestroyCommandPool)(VkDevice, VkCommandPool, const void*) = NULL;
static void (*vkDestroyDescriptorPool)(VkDevice, VkDescriptorPool, const void*) = NULL;
static void (*vkDeviceWaitIdle)(VkDevice) = NULL;

static int load_vulkan_symbols() {
#define LOAD(name) do { \
    *(void**)(&name) = dlsym(vk_lib, #name); \
    if (!name) return 0; \
} while(0)
    LOAD(vkCreateInstance);
    LOAD(vkDestroyInstance);
    LOAD(vkEnumeratePhysicalDevices);
    LOAD(vkGetPhysicalDeviceProperties);
    LOAD(vkGetPhysicalDeviceQueueFamilyProperties);
    LOAD(vkCreateDevice);
    LOAD(vkGetDeviceQueue);
    LOAD(vkDestroyDevice);
    LOAD(vkCreateBuffer);
    LOAD(vkGetBufferMemoryRequirements);
    LOAD(vkAllocateMemory);
    LOAD(vkBindBufferMemory);
    LOAD(vkMapMemory);
    LOAD(vkUnmapMemory);
    LOAD(vkFreeMemory);
    LOAD(vkDestroyBuffer);
    LOAD(vkCreateShaderModule);
    LOAD(vkDestroyShaderModule);
    LOAD(vkCreateDescriptorSetLayout);
    LOAD(vkCreatePipelineLayout);
    LOAD(vkCreateComputePipelines);
    LOAD(vkDestroyPipeline);
    LOAD(vkDestroyPipelineLayout);
    LOAD(vkDestroyDescriptorSetLayout);
    LOAD(vkCreateDescriptorPool);
    LOAD(vkAllocateDescriptorSets);
    LOAD(vkUpdateDescriptorSets);
    LOAD(vkCreateCommandPool);
    LOAD(vkAllocateCommandBuffers);
    LOAD(vkBeginCommandBuffer);
    LOAD(vkCmdBindPipeline);
    LOAD(vkCmdBindDescriptorSets);
    LOAD(vkCmdDispatch);
    LOAD(vkEndCommandBuffer);
    LOAD(vkQueueSubmit);
    LOAD(vkWaitForFences);
    LOAD(vkCreateFence);
    LOAD(vkDestroyFence);
    LOAD(vkDestroyCommandPool);
    LOAD(vkDestroyDescriptorPool);
    LOAD(vkDeviceWaitIdle);
    return 1;
#undef LOAD
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

static int brief_gpu_init_vulkan_inner();
static int brief_gpu_init_opencl_inner();

/// Initialize GPU compute runtime (Vulkan preferred, OpenCL fallback).
/// Returns 1 on success, 0 on failure (no GPU runtime available).
/// Safe to call multiple times; subsequent calls are no-ops.
int brief_gpu_init() {
    if (vk_initialized) return vk_available;

    vk_initialized = 1;

    // Try Vulkan first
    vk_lib = dlopen("libvulkan.so.1", RTLD_LAZY | RTLD_LOCAL);
    if (vk_lib) {
        if (load_vulkan_symbols()) {
            if (brief_gpu_init_vulkan_inner()) {
                vk_available = 1;
                return 1;
            }
            dlclose(vk_lib);
            vk_lib = NULL;
        } else {
            dlclose(vk_lib);
            vk_lib = NULL;
        }
    }

    // Vulkan failed — try OpenCL
    if (brief_gpu_init_opencl_inner()) {
        vk_available = 1;
        return 1;
    }

    return 0;
}

static int brief_gpu_init_vulkan_inner() {

    // Create Vulkan instance (no extensions needed for compute-only)
    struct { uint32_t version; uint32_t count; const char** names; } app_info = {
        .version = 0,  // VK_MAKE_VERSION(1, 0, 0)
        .count = 0,
        .names = NULL,
    };
    struct { const void* next; void* app_info; const char* layer_names; uint32_t layer_count; const char** ext_names; uint32_t ext_count; } create_info = {
        .next = NULL,
        .app_info = &app_info,
        .layer_names = NULL,
        .layer_count = 0,
        .ext_names = NULL,
        .ext_count = 0,
    };

    if (vkCreateInstance(&create_info, NULL, &vk_instance) != VK_SUCCESS) {
        dlclose(vk_lib);
        vk_lib = NULL;
        return 0;
    }

    // Enumerate physical devices and pick the first compute-capable one
    uint32_t physical_device_count = 0;
    vkEnumeratePhysicalDevices(vk_instance, &physical_device_count, NULL);
    if (physical_device_count == 0) {
        vkDestroyInstance(vk_instance, NULL);
        dlclose(vk_lib);
        vk_lib = NULL;
        return 0;
    }

    // Just use the first device for simplicity
    uint64_t physical_device = 0;
    vkEnumeratePhysicalDevices(vk_instance, &physical_device_count, &physical_device);

    // Get queue family with compute support
    uint32_t queue_family_count = 0;
    vkGetPhysicalDeviceQueueFamilyProperties((void*)(uintptr_t)physical_device, &queue_family_count, NULL);
    // We need at least one queue family — assume family 0 has compute
    vk_queue_family_index = 0;

    // Create logical device
    float queue_priority = 1.0f;
    struct { uint32_t queue_family; uint32_t queue_count; float* priorities; } queue_info = {
        .queue_family = vk_queue_family_index,
        .queue_count = 1,
        .priorities = &queue_priority,
    };
    struct { const void* next; uint32_t flags; uint32_t queue_count; void* queues; uint32_t enabled_layer_count; const char** enabled_layer_names; uint32_t enabled_ext_count; const char** enabled_ext_names; const void* features; } device_info = {
        .next = NULL,
        .flags = 0,
        .queue_count = 1,
        .queues = &queue_info,
        .enabled_layer_count = 0,
        .enabled_layer_names = NULL,
        .enabled_ext_count = 0,
        .enabled_ext_names = NULL,
        .features = NULL,
    };

    if (vkCreateDevice((void*)(uintptr_t)physical_device, &device_info, NULL, &vk_device) != VK_SUCCESS) {
        vkDestroyInstance(vk_instance, NULL);
        dlclose(vk_lib);
        vk_lib = NULL;
        return 0;
    }

    // Get the compute queue
    vkGetDeviceQueue(vk_device, vk_queue_family_index, 0, &vk_queue);

    // Create descriptor set layout — support up to MAX_GPU_BUFFERS storage buffers
    struct {
        uint32_t binding;
        uint32_t type;
        uint32_t count;
        uint32_t stage_flags;
        void* samplers;
    } binding_descs[MAX_GPU_BUFFERS];
    for (int i = 0; i < MAX_GPU_BUFFERS; i++) {
        binding_descs[i].binding = (uint32_t)i;
        binding_descs[i].type = 7;  // VK_DESCRIPTOR_TYPE_STORAGE_BUFFER
        binding_descs[i].count = 1;
        binding_descs[i].stage_flags = 0x20;  // VK_SHADER_STAGE_COMPUTE_BIT
        binding_descs[i].samplers = NULL;
    }
    struct { uint32_t count; void* bindings; } desc_layout_info = {
        .count = MAX_GPU_BUFFERS,
        .bindings = binding_descs,
    };
    vkCreateDescriptorSetLayout(vk_device, &desc_layout_info, NULL, &vk_desc_set_layout);

    // Create pipeline layout
    struct { uint32_t count; VkDescriptorSetLayout* layouts; uint32_t push_count; void* push_ranges; } pipe_layout_info = {
        .count = 1,
        .layouts = &vk_desc_set_layout,
        .push_count = 0,
        .push_ranges = NULL,
    };
    vkCreatePipelineLayout(vk_device, &pipe_layout_info, NULL, &vk_pipeline_layout);

    // Create descriptor pool — pool for MAX_GPU_BUFFERS storage buffer descriptors
    struct { uint32_t max_sets; uint32_t pool_size_count; void* pool_sizes; } desc_pool_info = {
        .max_sets = 1,
        .pool_size_count = 1,
        .pool_sizes = NULL,
    };
    uint32_t pool_sizes_data[2] = { 7, MAX_GPU_BUFFERS };  // type=STORAGE_BUFFER, count=MAX_GPU_BUFFERS
    desc_pool_info.pool_sizes = pool_sizes_data;
    vkCreateDescriptorPool(vk_device, &desc_pool_info, NULL, &vk_desc_pool);

    // Create command pool
    struct { uint32_t flags; uint32_t queue_family; } cmd_pool_info = {
        .flags = 0,
        .queue_family = vk_queue_family_index,
    };
    vkCreateCommandPool(vk_device, &cmd_pool_info, NULL, &vk_cmd_pool);

    // Allocate one command buffer
    struct { uint32_t level; uint32_t count; } cmd_alloc_info = {
        .level = 0,  // VK_COMMAND_BUFFER_LEVEL_PRIMARY
        .count = 1,
    };
    vkAllocateCommandBuffers(vk_device, &cmd_alloc_info, &vk_cmd_buf);

    // Create fence
    struct { uint32_t flags; } fence_info = { .flags = 0 };
    vkCreateFence(vk_device, &fence_info, NULL, &vk_fence);

    vk_available = 1;
    return 1;
}

/// Returns 1 if a GPU backend (Vulkan or OpenCL) is available and initialized, 0 otherwise.
int brief_gpu_is_available() {
    if (!vk_initialized && !cl_available) {
        brief_gpu_init();  // tries Vulkan first
        if (!vk_available) {
            brief_gpu_init_opencl_inner();  // fallback to OpenCL
        }
    }
    return vk_available || cl_available;
}

/// Allocate a GPU buffer of `bytes` size.
/// Returns a buffer handle (positive int64_t), or 0 on failure.
int64_t brief_gpu_malloc(size_t bytes) {
    if (!brief_gpu_is_available()) return 0;

    // Find a free slot
    int slot = -1;
    for (int i = 0; i < MAX_GPU_BUFFERS; i++) {
        if (!gpu_buffers[i].used) {
            slot = i;
            break;
        }
    }
    if (slot < 0) return 0;

    // Create buffer
    struct { void* next; uint32_t flags; uint64_t size; uint32_t usage; uint32_t sharing; uint32_t count; uint32_t* indices; } buf_info = {
        .next = NULL,
        .flags = 0,
        .size = bytes,
        .usage = 0x8000,  // VK_BUFFER_USAGE_STORAGE_BUFFER_BIT | VK_BUFFER_USAGE_TRANSFER_DST_BIT | VK_BUFFER_USAGE_TRANSFER_SRC_BIT
        .sharing = 0,     // VK_SHARING_MODE_EXCLUSIVE
        .count = 0,
        .indices = NULL,
    };
    // Enable transfer and storage
    buf_info.usage = 0x8000 | 0x4000 | 0x2000;  // STORAGE | TRANSFER_DST | TRANSFER_SRC
    if (vkCreateBuffer(vk_device, &buf_info, NULL, &gpu_buffers[slot].buffer) != VK_SUCCESS) {
        return 0;
    }

    // Get memory requirements
    struct { uint64_t size; uint64_t alignment; uint32_t memory_type_bits; } mem_reqs = {0};
    vkGetBufferMemoryRequirements(vk_device, gpu_buffers[slot].buffer, &mem_reqs);

    // Allocate memory (device-local, host-visible)
    struct { uint32_t type_count; void* types; } alloc_info = {
        .type_count = 1,
        .types = NULL,
    };
    // Use memory type index 0 (assume host-visible + device-local at index 0 or 1)
    // In a full implementation, we'd find the right memory type.
    uint32_t mem_type_index = 0;
    struct { uint32_t type_index; uint64_t allocation_size; } mem_alloc = {
        .type_index = mem_type_index,
        .allocation_size = mem_reqs.size,
    };
    if (vkAllocateMemory(vk_device, &mem_alloc, NULL, &gpu_buffers[slot].memory) != VK_SUCCESS) {
        vkDestroyBuffer(vk_device, gpu_buffers[slot].buffer, NULL);
        gpu_buffers[slot].buffer = (VkBuffer)0;
        return 0;
    }

    vkBindBufferMemory(vk_device, gpu_buffers[slot].buffer, gpu_buffers[slot].memory, 0);

    // Map memory for host access
    gpu_buffers[slot].host_ptr = vkMapMemory(vk_device, gpu_buffers[slot].memory, 0, bytes, 0);
    gpu_buffers[slot].size = bytes;
    gpu_buffers[slot].used = 1;

    int64_t handle = (int64_t)(uintptr_t)(size_t)(slot + 1);  // 1-based handle
    return handle;
}

/// Free a GPU buffer previously allocated with brief_gpu_malloc.
void brief_gpu_free(int64_t handle) {
    if (!vk_available || handle <= 0) return;
    int slot = (int)(handle - 1);
    if (slot < 0 || slot >= MAX_GPU_BUFFERS || !gpu_buffers[slot].used) return;

    vkUnmapMemory(vk_device, gpu_buffers[slot].memory);
    vkFreeMemory(vk_device, gpu_buffers[slot].memory, NULL);
    vkDestroyBuffer(vk_device, gpu_buffers[slot].buffer, NULL);
    gpu_buffers[slot].used = 0;
    gpu_buffers[slot].host_ptr = NULL;
    gpu_buffers[slot].memory = (VkDeviceMemory)0;
    gpu_buffers[slot].buffer = (VkBuffer)0;
    gpu_buffers[slot].size = 0;
}

/// Copy data between host and device.
/// dir: 0 = host→device, 1 = device→host
void brief_gpu_memcpy(int64_t dst_handle, int64_t src_handle, size_t bytes, int dir) {
    if (!vk_available) return;

    if (dir == 0) {
        // host → device: copy from src host pointer to dst device memory
        int dst_slot = (int)(dst_handle - 1);
        if (dst_slot < 0 || dst_slot >= MAX_GPU_BUFFERS || !gpu_buffers[dst_slot].used) return;
        void* src_ptr = (void*)(uintptr_t)src_handle;
        memcpy(gpu_buffers[dst_slot].host_ptr, src_ptr, bytes);
    } else {
        // device → host: copy from src device memory to dst host pointer
        int src_slot = (int)(src_handle - 1);
        if (src_slot < 0 || src_slot >= MAX_GPU_BUFFERS || !gpu_buffers[src_slot].used) return;
        void* dst_ptr = (void*)(uintptr_t)dst_handle;
        memcpy(dst_ptr, gpu_buffers[src_slot].host_ptr, bytes);
    }
}

/// Dispatch a compute shader.
/// `kernel_spirv` — pointer to SPIR-V binary data
/// `kernel_size` — size of the SPIR-V binary in bytes
/// `grid_x` — number of workgroups in X dimension
/// `block_x` — local workgroup size in X dimension
/// `buffer_handles` — array of buffer handle int64_ts
/// `num_buffers` — count of buffer handles

// Forward declarations for backend-specific dispatch functions
static void brief_gpu_launch_vulkan(
    const void* kernel_spirv, size_t kernel_size,
    int grid_x, int grid_y, int grid_z, int block_x,
    const int64_t* buffer_handles, int num_buffers
);
static void brief_gpu_launch_opencl(
    const void* kernel_spirv, size_t kernel_size,
    int grid_x, int grid_y, int grid_z, int block_x,
    const int64_t* buffer_handles, int num_buffers
);

void brief_gpu_launch(
    const void* kernel_spirv,
    size_t kernel_size,
    int grid_x,
    int grid_y,
    int grid_z,
    int block_x,
    const int64_t* buffer_handles,
    int num_buffers
) {
    if (vk_available) {
        brief_gpu_launch_vulkan(kernel_spirv, kernel_size, grid_x, grid_y, grid_z, block_x, buffer_handles, num_buffers);
        return;
    }
    if (cl_available) {
        brief_gpu_launch_opencl(kernel_spirv, kernel_size, grid_x, grid_y, grid_z, block_x, buffer_handles, num_buffers);
        return;
    }
}

// ── Vulkan dispatch (primary) ──────────────────────────────────

static void brief_gpu_launch_vulkan(
    const void* kernel_spirv,
    size_t kernel_size,
    int grid_x,
    int grid_y,
    int grid_z,
    int block_x,
    const int64_t* buffer_handles,
    int num_buffers
) {
    // Create shader module from SPIR-V
    struct { void* next; uint32_t flags; size_t code_size; const uint32_t* code; } shader_info = {
        .next = NULL,
        .flags = 0,
        .code_size = kernel_size,
        .code = (const uint32_t*)kernel_spirv,
    };
    VkShaderModule shader_module;
    if (vkCreateShaderModule(vk_device, &shader_info, NULL, &shader_module) != VK_SUCCESS) {
        return;
    }

    // Create compute pipeline
    struct { uint32_t stage; VkShaderModule module; void* name; void* specialization; } stage_info = {
        .stage = 0x20,  // VK_SHADER_STAGE_COMPUTE_BIT
        .module = shader_module,
        .name = "main",
        .specialization = NULL,
    };
    struct { void* next; uint32_t flags; void* stage; uint32_t stage_count; void* layout; } pipeline_info = {
        .next = NULL,
        .flags = 0,
        .stage = &stage_info,
        .stage_count = 1,
        .layout = &vk_pipeline_layout,
    };
    VkPipeline pipeline;
    if (vkCreateComputePipelines(vk_device, VK_NULL_HANDLE, 1, &pipeline_info, NULL, &pipeline) != VK_SUCCESS) {
        vkDestroyShaderModule(vk_device, shader_module, NULL);
        return;
    }

    // Write descriptor set
    VkDescriptorSet desc_set;
    struct { void* next; VkDescriptorPool pool; uint32_t count; VkDescriptorSet* sets; } desc_alloc = {
        .next = NULL,
        .pool = vk_desc_pool,
        .count = 1,
        .sets = &desc_set,
    };
    vkAllocateDescriptorSets(vk_device, &desc_alloc, &desc_set);

    // Build descriptor buffer infos and write entries for each buffer handle.
    struct { uint64_t buffer; uint64_t offset; uint64_t range; } buf_infos[MAX_GPU_BUFFERS];
    struct {
        void* next;
        uint32_t dst_set;
        uint32_t dst_binding;
        uint32_t dst_array_element;
        uint32_t descriptor_count;
        uint32_t descriptor_type;
        void* p_buffer_info;
        void* p_tex_info;
        void* p_tex_view;
    } write_descs[MAX_GPU_BUFFERS];
    int num_valid = 0;
    for (int i = 0; i < num_buffers && i < MAX_GPU_BUFFERS; i++) {
        int slot = (int)(buffer_handles[i] - 1);
        if (slot < 0 || slot >= MAX_GPU_BUFFERS || !gpu_buffers[slot].used) continue;
        buf_infos[num_valid].buffer = (uint64_t)(uintptr_t)&gpu_buffers[slot];
        buf_infos[num_valid].offset = 0;
        buf_infos[num_valid].range = gpu_buffers[slot].size;
        write_descs[num_valid].next = NULL;
        write_descs[num_valid].dst_set = (uint32_t)(uintptr_t)desc_set;
        write_descs[num_valid].dst_binding = (uint32_t)i;
        write_descs[num_valid].dst_array_element = 0;
        write_descs[num_valid].descriptor_count = 1;
        write_descs[num_valid].descriptor_type = 7;  // VK_DESCRIPTOR_TYPE_STORAGE_BUFFER
        write_descs[num_valid].p_buffer_info = &buf_infos[num_valid];
        write_descs[num_valid].p_tex_info = NULL;
        write_descs[num_valid].p_tex_view = NULL;
        num_valid++;
    }
    if (num_valid > 0) {
        vkUpdateDescriptorSets(vk_device, (uint32_t)num_valid, write_descs, 0, NULL);
    }

    // Record command buffer
    struct { uint32_t flags; void* inheritance; } begin_info = { .flags = 0, .inheritance = NULL };
    vkBeginCommandBuffer(vk_cmd_buf, &begin_info);

    vkCmdBindPipeline(vk_cmd_buf, 0x4000, pipeline);  // VK_PIPELINE_BIND_POINT_COMPUTE
    vkCmdDispatch(vk_cmd_buf, (uint32_t)grid_x, (uint32_t)grid_y, (uint32_t)grid_z);

    vkEndCommandBuffer(vk_cmd_buf);

    // Submit
    struct { uint32_t count; VkCommandBuffer* bufs; } submit_info = {
        .count = 1,
        .bufs = &vk_cmd_buf,
    };
    // Simplified — would use proper VkSubmitInfo structure
    vkQueueSubmit(vk_queue, 1, &submit_info, vk_fence);

    // Wait for completion
    vkWaitForFences(vk_device, 1, &vk_fence, 1, 1000000000UL);  // 1 second timeout
    vkDestroyFence(vk_device, vk_fence, NULL);

    // Reset fence for next use
    struct { uint32_t flags; } fence_info = { .flags = 0 };
    vkCreateFence(vk_device, &fence_info, NULL, &vk_fence);

    // Cleanup pipeline and shader module
    vkDestroyPipeline(vk_device, pipeline, NULL);
    vkDestroyShaderModule(vk_device, shader_module, NULL);
}

// ── OpenCL dispatch (fallback when Vulkan is unavailable) ──────

static void brief_gpu_launch_opencl(
    const void* kernel_spirv,
    size_t kernel_size,
    int grid_x,
    int grid_y,
    int grid_z,
    int block_x,
    const int64_t* buffer_handles,
    int num_buffers
) {
    int err;

    // Create program from SPIR-V IL
    cl_program program = clCreateProgramWithIL(cl_ctx, kernel_spirv, kernel_size, &err);
    if (!program || err != 0) return;

    // Build the program
    err = clBuildProgram(program, 0, NULL, "", NULL, NULL);
    if (err != 0) {
        // Build failure — try to get build log for diagnostics
        char log_buf[4096];
        size_t log_size = 0;
        if (clGetProgramBuildInfo(program, cl_device, 0x1000, sizeof(log_buf), log_buf, &log_size) == 0) {
            (void)log_buf;  // diagnostics available if needed
        }
        clReleaseProgram(program);
        return;
    }

    // Create kernel
    cl_kernel kernel = clCreateKernel(program, "main", &err);
    if (!kernel || err != 0) { clReleaseProgram(program); return; }

    // Total work items = grid * block_x (per dimension)
    size_t global_size[3];
    global_size[0] = (size_t)grid_x * (size_t)(block_x > 0 ? block_x : 1);
    global_size[1] = (size_t)(grid_y > 0 ? grid_y : 1);
    global_size[2] = (size_t)(grid_z > 0 ? grid_z : 1);

    // Local work size
    size_t local_size[3] = {
        (size_t)(block_x > 0 ? block_x : 64),
        1, 1
    };

    // Copy host data to OpenCL buffers and set kernel args
    // The SPIR-V kernel signature is: kernel(i8* %in_buf, i8* %out_buf, i64 %N)
    // where arg 0 = input buffer, arg 1 = output buffer, arg 2 = N (total elements)
    cl_mem cl_bufs[MAX_GPU_BUFFERS];
    int num_cl_bufs = 0;

    for (int i = 0; i < num_buffers && i < MAX_GPU_BUFFERS; i++) {
        int slot = (int)(buffer_handles[i] - 1);
        if (slot < 0 || slot >= MAX_GPU_BUFFERS || !gpu_buffers[slot].used) continue;

        // Create OpenCL buffer using the host pointer data
        cl_bufs[num_cl_bufs] = clCreateBuffer(cl_ctx, 3,  // CL_MEM_READ_WRITE
            gpu_buffers[slot].size, NULL, &err);
        if (!cl_bufs[num_cl_bufs] || err != 0) {
            for (int j = 0; j < num_cl_bufs; j++) clReleaseMemObject(cl_bufs[j]);
            clReleaseKernel(kernel);
            clReleaseProgram(program);
            return;
        }

        // Write host data to device buffer before dispatch
        clEnqueueWriteBuffer(cl_queue, cl_bufs[num_cl_bufs], 0, 0,
            gpu_buffers[slot].size, gpu_buffers[slot].host_ptr, 0, NULL, NULL);

        // Set kernel arg — bind to the kernel parameter index i
        err = clSetKernelArg(kernel, (uint32_t)i, sizeof(cl_mem), &cl_bufs[num_cl_bufs]);
        if (err != 0) {
            for (int j = 0; j <= num_cl_bufs; j++) clReleaseMemObject(cl_bufs[j]);
            clReleaseKernel(kernel);
            clReleaseProgram(program);
            return;
        }
        num_cl_bufs++;
    }

    // Set the N parameter (total element count) as the next kernel arg
    int64_t total_n = (int64_t)global_size[0] * (int64_t)global_size[1] * (int64_t)global_size[2];
    err = clSetKernelArg(kernel, (uint32_t)num_cl_bufs, sizeof(int64_t), &total_n);
    if (err != 0) {
        for (int j = 0; j < num_cl_bufs; j++) clReleaseMemObject(cl_bufs[j]);
        clReleaseKernel(kernel);
        clReleaseProgram(program);
        return;
    }

    // Dispatch
    err = clEnqueueNDRangeKernel(cl_queue, kernel, 3, NULL, global_size, local_size, 0, NULL, NULL);
    if (err != 0) {
        for (int j = 0; j < num_cl_bufs; j++) clReleaseMemObject(cl_bufs[j]);
        clReleaseKernel(kernel);
        clReleaseProgram(program);
        return;
    }

    // Wait for completion
    clFinish(cl_queue);

    // Read data back from device to host
    for (int i = 0; i < num_cl_bufs; i++) {
        int slot = (int)(buffer_handles[i] - 1);
        if (slot < 0 || slot >= MAX_GPU_BUFFERS || !gpu_buffers[slot].used) continue;
        clEnqueueReadBuffer(cl_queue, cl_bufs[i], 0, 0,
            gpu_buffers[slot].size, gpu_buffers[slot].host_ptr, 0, NULL, NULL);
    }
    clFinish(cl_queue);

    // Cleanup
    for (int i = 0; i < num_cl_bufs; i++) clReleaseMemObject(cl_bufs[i]);
    clReleaseKernel(kernel);
    clReleaseProgram(program);
}

/// Shutdown the GPU runtime, releasing all Vulkan resources.
void brief_gpu_shutdown() {
    if (!vk_available) return;

    // Free all buffers
    for (int i = 0; i < MAX_GPU_BUFFERS; i++) {
        if (gpu_buffers[i].used) {
            brief_gpu_free((int64_t)(i + 1));
        }
    }

    vkDestroyFence(vk_device, vk_fence, NULL);
    vkDestroyCommandPool(vk_device, vk_cmd_pool, NULL);
    vkDestroyDescriptorPool(vk_device, vk_desc_pool, NULL);
    vkDestroyPipelineLayout(vk_device, vk_pipeline_layout, NULL);
    vkDestroyDescriptorSetLayout(vk_device, vk_desc_set_layout, NULL);
    vkDeviceWaitIdle(vk_device);
    vkDestroyDevice(vk_device, NULL);
    vkDestroyInstance(vk_instance, NULL);

    if (vk_lib) {
        dlclose(vk_lib);
        vk_lib = NULL;
    }

    vk_available = 0;
    vk_initialized = 0;
}

// ---------------------------------------------------------------------------
// OpenCL backend (fallback when Vulkan is unavailable)
// ---------------------------------------------------------------------------
// SPIR-V is consumed natively by OpenCL via clCreateProgramWithIL.
// This path is used when --gpu-backend includes 'opencl' and Vulkan init
// failed.
// (Types and function pointers declared above alongside the Vulkan state.)

static int load_opencl_symbols() {
#define CL_LOAD(name) do { \
    *(void**)(&name) = dlsym(cl_lib, #name); \
    if (!name) return 0; \
} while(0)
    CL_LOAD(clGetPlatformIDs);
    CL_LOAD(clGetDeviceIDs);
    CL_LOAD(clCreateContext);
    CL_LOAD(clCreateCommandQueue);
    CL_LOAD(clCreateProgramWithIL);
    CL_LOAD(clBuildProgram);
    CL_LOAD(clCreateKernel);
    CL_LOAD(clSetKernelArgSVMPointer);
    CL_LOAD(clEnqueueNDRangeKernel);
    CL_LOAD(clEnqueueReadBuffer);
    CL_LOAD(clEnqueueWriteBuffer);
    CL_LOAD(clFinish);
    CL_LOAD(clReleaseKernel);
    CL_LOAD(clReleaseProgram);
    CL_LOAD(clReleaseCommandQueue);
    CL_LOAD(clReleaseContext);
    CL_LOAD(clReleaseMemObject);
    CL_LOAD(clSVMAlloc);
    CL_LOAD(clSVMFree);
    CL_LOAD(clCreateBuffer);
    CL_LOAD(clSetKernelArg);
    CL_LOAD(clGetProgramBuildInfo);
    return 1;
#undef CL_LOAD
}

static int brief_gpu_init_opencl_inner() {
    cl_lib = dlopen("libOpenCL.so.1", RTLD_LAZY | RTLD_LOCAL);
    if (!cl_lib) return 0;
    if (!load_opencl_symbols()) { dlclose(cl_lib); cl_lib = NULL; return 0; }

    uint32_t num_platforms = 0;
    if (clGetPlatformIDs(0, NULL, &num_platforms) != 0 || num_platforms == 0) {
        dlclose(cl_lib); cl_lib = NULL; return 0;
    }
    if (clGetPlatformIDs(1, &cl_platform, NULL) != 0) {
        dlclose(cl_lib); cl_lib = NULL; return 0;
    }

    uint32_t num_devices = 0;
    if (clGetDeviceIDs(cl_platform, (uint64_t)1 << 4, 0, NULL, &num_devices) != 0 || num_devices == 0) {
        dlclose(cl_lib); cl_lib = NULL; return 0;
    }
    if (clGetDeviceIDs(cl_platform, (uint64_t)1 << 4, 1, &cl_device, NULL) != 0) {
        dlclose(cl_lib); cl_lib = NULL; return 0;
    }

    int err = 0;
    cl_ctx = clCreateContext(NULL, 1, &cl_device, NULL, NULL, &err);
    if (!cl_ctx || err != 0) { dlclose(cl_lib); cl_lib = NULL; return 0; }

    cl_queue = clCreateCommandQueue(cl_ctx, cl_device, 0, &err);
    if (!cl_queue || err != 0) { clReleaseContext(cl_ctx); dlclose(cl_lib); cl_lib = NULL; return 0; }

    cl_available = 1;
    return 1;
}

// ── CPU fallback stubs for GPU intrinsics ──────────────────────
//
// These are called by the CPU codegen path when a program uses GPU
// intrinsics (get_global_id#, barrier#, etc.) but runs on CPU.
// All return single-thread default values.

int64_t __get_global_id(int32_t dim) {
    (void)dim;
    return 0;  // single-thread CPU: always thread 0
}

int64_t __get_local_id(int32_t dim) {
    (void)dim;
    return 0;
}

int64_t __get_group_id(int32_t dim) {
    (void)dim;
    return 0;
}

int64_t __get_num_groups(int32_t dim) {
    (void)dim;
    return 1;  // one workgroup on CPU
}

void __barrier__(void) {
    // no-op on single-thread CPU
}


