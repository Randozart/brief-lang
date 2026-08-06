// Vulkan device driver for briv_accel_rt — SPIR-V via Vulkan compute.
//
// Ported from the legacy briv_gpu_rt.c Vulkan backend and restructured to the
// single-flat-buffer model: one host-visible STORAGE_BUFFER holds the kernel's
// packed `%State` projection; the kernel entry is `main`. Loaded via
// dlopen("libvulkan.so.1"); when absent, available() returns 0 and the chain
// falls back to OpenCL then CPU.
//
// HARDENING NOTE: this carries over the legacy mechanism and its known
// simplifications (host-visible memory type 0, minimal synchronization) — it
// is the seed of the formalized driver, to be hardened against real hardware
// (proper memory-type selection + staging) before it is trusted for speed.

#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <dlfcn.h>

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

#define VK_SUCCESS 0
#define VK_NULL_HANDLE 0
#define VK_SHADER_STAGE_COMPUTE_BIT 0x20u
#define VK_BUFFER_USAGE_STORAGE_BUFFER_BIT 0x80u
#define VK_BUFFER_USAGE_TRANSFER_SRC_BIT 0x2000u
#define VK_BUFFER_USAGE_TRANSFER_DST_BIT 0x4000u
#define VK_DESCRIPTOR_TYPE_STORAGE_BUFFER 7u
#define VK_PIPELINE_BIND_POINT_COMPUTE 0x4000u

static void* vk_lib = NULL;
static int vk_ready = 0;
static VkInstance vk_instance;
static VkDevice vk_device;
static VkPipelineLayout vk_pipeline_layout;
static VkDescriptorSetLayout vk_desc_set_layout;
static VkDescriptorPool vk_desc_pool;
static VkCommandPool vk_cmd_pool;
static VkCommandBuffer vk_cmd_buf;
static VkFence vk_fence;
static uint32_t vk_queue_family_index = 0;
static uint64_t vk_queue = 0;

static int (*vkCreateInstance)(const void*, const void*, VkInstance*) = NULL;
static void (*vkDestroyInstance)(VkInstance, const void*) = NULL;
static int (*vkEnumeratePhysicalDevices)(VkInstance, uint32_t*, void*) = NULL;
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

static int load_vulkan_symbols(void) {
#define LOAD(name) do { *(void**)(&name) = dlsym(vk_lib, #name); if (!name) return 0; } while (0)
    LOAD(vkCreateInstance);
    LOAD(vkDestroyInstance);
    LOAD(vkEnumeratePhysicalDevices);
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

static int briv_dev_vulkan_available(void) {
    if (vk_ready) {
        return 1;
    }
    vk_lib = dlopen("libvulkan.so.1", RTLD_LAZY | RTLD_LOCAL);
    if (!vk_lib) {
        return 0;
    }
    if (!load_vulkan_symbols()) {
        dlclose(vk_lib);
        vk_lib = NULL;
        return 0;
    }
    vk_ready = 1;
    return 1;
}

static int briv_dev_vulkan_init(void) {
    if (!briv_dev_vulkan_available()) {
        return 0;
    }
    // Instance (compute-only; no extensions).
    struct { uint32_t version; uint32_t count; const char** names; } app_info = {
        .version = 0, .count = 0, .names = NULL,
    };
    struct { const void* next; void* app_info; const char* layer_names;
             uint32_t layer_count; const char** ext_names; uint32_t ext_count; } create_info = {
        .next = NULL, .app_info = &app_info, .layer_names = NULL,
        .layer_count = 0, .ext_names = NULL, .ext_count = 0,
    };
    if (vkCreateInstance(&create_info, NULL, &vk_instance) != VK_SUCCESS) {
        dlclose(vk_lib);
        vk_lib = NULL;
        return 0;
    }
    uint32_t pdc = 0;
    vkEnumeratePhysicalDevices(vk_instance, &pdc, NULL);
    if (pdc == 0) {
        vkDestroyInstance(vk_instance, NULL);
        dlclose(vk_lib);
        vk_lib = NULL;
        return 0;
    }
    uint64_t physical_device = 0;
    vkEnumeratePhysicalDevices(vk_instance, &pdc, &physical_device);
    // HARDENING: legacy assumes family 0 has compute.
    vk_queue_family_index = 0;
    float queue_priority = 1.0f;
    struct { uint32_t queue_family; uint32_t queue_count; float* priorities; } queue_info = {
        .queue_family = vk_queue_family_index, .queue_count = 1, .priorities = &queue_priority,
    };
    struct { const void* next; uint32_t flags; uint32_t queue_count; void* queues;
             uint32_t enabled_layer_count; const char** enabled_layer_names;
             uint32_t enabled_ext_count; const char** enabled_ext_names; const void* features; } device_info = {
        .next = NULL, .flags = 0, .queue_count = 1, .queues = &queue_info,
        .enabled_layer_count = 0, .enabled_layer_names = NULL,
        .enabled_ext_count = 0, .enabled_ext_names = NULL, .features = NULL,
    };
    if (vkCreateDevice((void*)(uintptr_t)physical_device, &device_info, NULL, &vk_device) != VK_SUCCESS) {
        vkDestroyInstance(vk_instance, NULL);
        dlclose(vk_lib);
        vk_lib = NULL;
        return 0;
    }
    vkGetDeviceQueue(vk_device, vk_queue_family_index, 0, &vk_queue);

    // One STORAGE_BUFFER binding (binding 0) = the flat %State projection.
    struct { uint32_t binding; uint32_t type; uint32_t count; uint32_t stage_flags; void* samplers; } binding_desc = {
        .binding = 0, .type = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, .count = 1,
        .stage_flags = VK_SHADER_STAGE_COMPUTE_BIT, .samplers = NULL,
    };
    struct { uint32_t count; void* bindings; } desc_layout_info = {
        .count = 1, .bindings = &binding_desc,
    };
    vkCreateDescriptorSetLayout(vk_device, &desc_layout_info, NULL, &vk_desc_set_layout);
    struct { uint32_t count; VkDescriptorSetLayout* layouts; uint32_t push_count; void* push_ranges; } pipe_layout_info = {
        .count = 1, .layouts = &vk_desc_set_layout, .push_count = 0, .push_ranges = NULL,
    };
    vkCreatePipelineLayout(vk_device, &pipe_layout_info, NULL, &vk_pipeline_layout);
    uint32_t pool_sizes_data[2] = { VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, 1 };
    struct { uint32_t max_sets; uint32_t pool_size_count; void* pool_sizes; } desc_pool_info = {
        .max_sets = 1, .pool_size_count = 1, .pool_sizes = pool_sizes_data,
    };
    vkCreateDescriptorPool(vk_device, &desc_pool_info, NULL, &vk_desc_pool);
    struct { uint32_t flags; uint32_t queue_family; } cmd_pool_info = {
        .flags = 0, .queue_family = vk_queue_family_index,
    };
    vkCreateCommandPool(vk_device, &cmd_pool_info, NULL, &vk_cmd_pool);
    struct { uint32_t level; uint32_t count; } cmd_alloc_info = {
        .level = 0, .count = 1,
    };
    vkAllocateCommandBuffers(vk_device, &cmd_alloc_info, &vk_cmd_buf);
    struct { uint32_t flags; } fence_info = { .flags = 0 };
    vkCreateFence(vk_device, &fence_info, NULL, &vk_fence);
    return 1;
}

typedef struct {
    VkShaderModule module;
    VkPipeline pipeline;
} BrivVulkanKernel;

static int briv_dev_vulkan_create_kernel(const uint8_t* spirv, size_t size, void** out) {
    struct { void* next; uint32_t flags; size_t code_size; const uint32_t* code; } shader_info = {
        .next = NULL, .flags = 0, .code_size = size, .code = (const uint32_t*)spirv,
    };
    VkShaderModule module;
    if (vkCreateShaderModule(vk_device, &shader_info, NULL, &module) != VK_SUCCESS) {
        return 0;
    }
    struct { uint32_t stage; VkShaderModule module; void* name; void* specialization; } stage_info = {
        .stage = VK_SHADER_STAGE_COMPUTE_BIT, .module = module, .name = "main", .specialization = NULL,
    };
    struct { void* next; uint32_t flags; void* stage; uint32_t stage_count; void* layout; } pipeline_info = {
        .next = NULL, .flags = 0, .stage = &stage_info, .stage_count = 1, .layout = &vk_pipeline_layout,
    };
    VkPipeline pipeline;
    if (vkCreateComputePipelines(vk_device, VK_NULL_HANDLE, 1, &pipeline_info, NULL, &pipeline) != VK_SUCCESS) {
        vkDestroyShaderModule(vk_device, module, NULL);
        return 0;
    }
    BrivVulkanKernel* k = calloc(1, sizeof(BrivVulkanKernel));
    if (!k) {
        vkDestroyPipeline(vk_device, pipeline, NULL);
        vkDestroyShaderModule(vk_device, module, NULL);
        return 0;
    }
    k->module = module;
    k->pipeline = pipeline;
    *out = k;
    return 1;
}

static int briv_dev_vulkan_launch(void* handle, const void* proj, size_t proj_bytes,
                                  size_t global_n, void* proj_out) {
    BrivVulkanKernel* k = (BrivVulkanKernel*)handle;

    // Create the single storage buffer (host-visible; HARDENING: memory type 0).
    VkBuffer buffer;
    struct { void* next; uint32_t flags; uint64_t size; uint32_t usage;
             uint32_t sharing; uint32_t count; uint32_t* indices; } buf_info = {
        .next = NULL, .flags = 0, .size = proj_bytes,
        .usage = VK_BUFFER_USAGE_STORAGE_BUFFER_BIT | VK_BUFFER_USAGE_TRANSFER_DST_BIT | VK_BUFFER_USAGE_TRANSFER_SRC_BIT,
        .sharing = 0, .count = 0, .indices = NULL,
    };
    if (vkCreateBuffer(vk_device, &buf_info, NULL, &buffer) != VK_SUCCESS) {
        return 0;
    }
    struct { uint64_t size; uint64_t alignment; uint32_t memory_type_bits; } mem_reqs = {0};
    vkGetBufferMemoryRequirements(vk_device, buffer, &mem_reqs);
    VkDeviceMemory memory;
    struct { uint32_t type_index; uint64_t allocation_size; } mem_alloc = {
        .type_index = 0, .allocation_size = mem_reqs.size,
    };
    if (vkAllocateMemory(vk_device, &mem_alloc, NULL, &memory) != VK_SUCCESS) {
        vkDestroyBuffer(vk_device, buffer, NULL);
        return 0;
    }
    vkBindBufferMemory(vk_device, buffer, memory, 0);
    void* host_ptr = vkMapMemory(vk_device, memory, 0, proj_bytes, 0);
    memcpy(host_ptr, proj, proj_bytes);

    // Allocate one descriptor set, bind the buffer at binding 0.
    VkDescriptorSet desc_set;
    struct { void* next; VkDescriptorPool pool; uint32_t count; VkDescriptorSet* sets; } desc_alloc = {
        .next = NULL, .pool = vk_desc_pool, .count = 1, .sets = &desc_set,
    };
    vkAllocateDescriptorSets(vk_device, &desc_alloc, &desc_set);
    struct { uint64_t buffer; uint64_t offset; uint64_t range; } buf_info_desc = {
        .buffer = (uint64_t)(uintptr_t)buffer, .offset = 0, .range = proj_bytes,
    };
    struct { void* next; uint32_t dst_set; uint32_t dst_binding; uint32_t dst_array_element;
             uint32_t descriptor_count; uint32_t descriptor_type; void* p_buffer_info;
             void* p_tex_info; void* p_tex_view; } write_desc = {
        .next = NULL, .dst_set = (uint32_t)(uintptr_t)desc_set, .dst_binding = 0,
        .dst_array_element = 0, .descriptor_count = 1,
        .descriptor_type = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER,
        .p_buffer_info = &buf_info_desc, .p_tex_info = NULL, .p_tex_view = NULL,
    };
    vkUpdateDescriptorSets(vk_device, 1, &write_desc, 0, NULL);

    struct { uint32_t flags; void* inheritance; } begin_info = { .flags = 0, .inheritance = NULL };
    vkBeginCommandBuffer(vk_cmd_buf, &begin_info);
    vkCmdBindPipeline(vk_cmd_buf, VK_PIPELINE_BIND_POINT_COMPUTE, k->pipeline);
    vkCmdBindDescriptorSets(vk_cmd_buf, VK_PIPELINE_BIND_POINT_COMPUTE, vk_pipeline_layout,
                            0, 1, &desc_set, 0, NULL);
    vkCmdDispatch(vk_cmd_buf, (uint32_t)global_n, 1, 1);
    vkEndCommandBuffer(vk_cmd_buf);

    struct { uint32_t count; VkCommandBuffer* bufs; } submit_info = { .count = 1, .bufs = &vk_cmd_buf };
    vkQueueSubmit(vk_queue, 1, &submit_info, vk_fence);
    vkWaitForFences(vk_device, 1, &vk_fence, 1, 1000000000UL);
    vkDestroyFence(vk_device, vk_fence, NULL);
    struct { uint32_t flags; } fence_info = { .flags = 0 };
    vkCreateFence(vk_device, &fence_info, NULL, &vk_fence);

    memcpy(proj_out, host_ptr, proj_bytes);
    vkUnmapMemory(vk_device, memory);
    vkFreeMemory(vk_device, memory, NULL);
    vkDestroyBuffer(vk_device, buffer, NULL);
    return 1;
}

static void briv_dev_vulkan_destroy_kernel(void* handle) {
    BrivVulkanKernel* k = (BrivVulkanKernel*)handle;
    if (!k) {
        return;
    }
    vkDestroyPipeline(vk_device, k->pipeline, NULL);
    vkDestroyShaderModule(vk_device, k->module, NULL);
    free(k);
}

static void briv_dev_vulkan_shutdown(void) {
    if (vk_device) {
        vkDeviceWaitIdle(vk_device);
        vkDestroyFence(vk_device, vk_fence, NULL);
        vkDestroyCommandPool(vk_device, vk_cmd_pool, NULL);
        vkDestroyDescriptorPool(vk_device, vk_desc_pool, NULL);
        vkDestroyPipelineLayout(vk_device, vk_pipeline_layout, NULL);
        vkDestroyDescriptorSetLayout(vk_device, vk_desc_set_layout, NULL);
        vkDestroyDevice(vk_device, NULL);
    }
    if (vk_instance) {
        vkDestroyInstance(vk_instance, NULL);
    }
    if (vk_lib) {
        dlclose(vk_lib);
        vk_lib = NULL;
    }
    vk_ready = 0;
}

BrivDeviceDriver briv_dev_vulkan = {
    "vulkan",
    0,  // capabilities: host-visible buffer copies, no zero-copy
    briv_dev_vulkan_available,
    briv_dev_vulkan_init,
    briv_dev_vulkan_create_kernel,
    briv_dev_vulkan_launch,
    briv_dev_vulkan_destroy_kernel,
    briv_dev_vulkan_shutdown,
};
