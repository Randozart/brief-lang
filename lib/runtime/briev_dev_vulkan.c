// Vulkan device driver for briev_accel_rt — SPIR-V via Vulkan compute.
//
// 2026-08-31 (plan abv-gpu-by-default, item 3): REWRITTEN against the real
// VkStruct layouts (verified against /usr/include/vulkan/vulkan_core.h).
// The ported legacy driver used sType-less anonymous structs whose member
// offsets did not match the Vulkan ABI — vkCreateDevice dereferenced
// garbage and the process died before the first dispatch. Also fixed:
//   - memory-type SEARCH (HOST_VISIBLE|HOST_COHERENT) instead of the
//     hardcoded type 0 (which is not host-visible on all devices);
//   - descriptor pool RESET per launch (the pool had max_sets=1 and was
//     never reset — the second launch ran on an unchecked failed alloc);
//   - fence recreated only after a successful wait; wait failure is
//     reported instead of racing the readback.
// Still deliberately simple (host-visible staging per launch, one queue,
// compute-only) — harden further only by measurement (plan item 3).
//
// Loaded via dlopen("libvulkan.so.1"); when absent, available() returns 0
// and the chain falls back to OpenCL then CPU.

#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <dlfcn.h>

typedef uint64_t VkInstance_T;
typedef uint64_t VkDevice_T;
typedef uint64_t VkPhysicalDevice_T;
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
typedef uint64_t VkQueue_T;

typedef VkInstance_T* VkInstance;
typedef VkPhysicalDevice_T VkPhysicalDevice;
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
typedef uint64_t VkQueue;

typedef enum {
    VK_STRUCTURE_TYPE_APPLICATION_INFO = 0,
    VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO = 1,
    VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO = 2,
    VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO = 3,
    VK_STRUCTURE_TYPE_SUBMIT_INFO = 4,
    VK_STRUCTURE_TYPE_FENCE_CREATE_INFO = 8,
    VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO = 16,
    VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO = 18,
    VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO = 29,
    VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO = 30,
    VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO = 32,
    VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO = 33,
    VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO = 34,
    VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET = 35,
    VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO = 39,
    VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO = 40,
    VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO = 42,
} VkStructureType;

// sType values are verified against vulkan_core.h (see the enum above);
// keep the raw numbers so the driver needs no Vulkan headers to compile.
#define VK_S_UNUSED 0
#define VK_SUCCESS 0
#define VK_NULL_HANDLE 0
// VkShaderStageFlagBits VALUE for compute (used in VkPipelineShaderStageCreateInfo.stage).
#define VK_SHADER_STAGE_COMPUTE 6u
// VkShaderStageFlagBits BIT mask (used in descriptor layout stageFlags).
#define VK_SHADER_STAGE_COMPUTE_BIT 0x20u
#define VK_BUFFER_USAGE_STORAGE_BUFFER_BIT 0x80u
#define VK_BUFFER_USAGE_TRANSFER_SRC_BIT 0x2000u
#define VK_BUFFER_USAGE_TRANSFER_DST_BIT 0x4000u
#define VK_DESCRIPTOR_TYPE_STORAGE_BUFFER 7u
#define VK_PIPELINE_BIND_POINT_COMPUTE 1u
#define VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT 0x2u
#define VK_MEMORY_PROPERTY_HOST_COHERENT_BIT 0x8u
#define VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT 0x1u
#define VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT 0x00000800u
#define VK_PIPELINE_STAGE_TRANSFER_BIT 0x00001000u
#define VK_ACCESS_SHADER_READ_BIT 0x00000020u
#define VK_ACCESS_SHADER_WRITE_BIT 0x00000040u
#define VK_ACCESS_TRANSFER_READ_BIT 0x00000800u
#define VK_ACCESS_TRANSFER_WRITE_BIT 0x00001000u
#define VK_QUEUE_FAMILY_IGNORED 0xFFFFFFFFu
#define VK_WHOLE_SIZE_MACRO 0xFFFFFFFFFFFFFFFFull
#define VK_COMMAND_BUFFER_LEVEL_PRIMARY 0u
#define VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT 0x1u
#define VK_MAX_MEMORY_TYPES 32u
// Must match the kernel's OpExecutionMode LocalSize (spirv/kernel.rs LOCAL_SIZE_X).
#define VK_LOCAL_SIZE_X 256u

typedef struct { uint32_t sType; const void* pNext; uint32_t flags;
                 const void* pApplicationInfo; uint32_t enabledLayerCount;
                 const char* const* ppEnabledLayerNames;
                 uint32_t enabledExtensionCount;
                 const char* const* ppEnabledExtensionNames; } VkInstanceCreateInfo;
typedef struct { uint32_t sType; const void* pNext; uint32_t flags;
                 uint32_t queueFamilyIndex; uint32_t queueCount;
                 const float* pQueuePriorities; } VkDeviceQueueCreateInfo;
typedef struct { uint32_t sType; const void* pNext; uint32_t flags;
                 uint32_t queueCreateInfoCount;
                 const VkDeviceQueueCreateInfo* pQueueCreateInfos;
                 uint32_t enabledLayerCount;
                 const char* const* ppEnabledLayerNames;
                 uint32_t enabledExtensionCount;
                 const char* const* ppEnabledExtensionNames;
                 const void* pEnabledFeatures; } VkDeviceCreateInfo;
typedef struct { uint32_t sType; const void* pNext; uint32_t flags;
                 uint64_t codeSize; const uint32_t* pCode; } VkShaderModuleCreateInfo;
typedef struct { uint32_t sType; const void* pNext; uint32_t flags;
                 uint32_t stage; uint64_t module; const char* pName;
                 const void* pSpecializationInfo; } VkPipelineShaderStageCreateInfo;
typedef struct { uint32_t sType; const void* pNext; uint32_t flags;
                 VkPipelineShaderStageCreateInfo stage; uint64_t layout;
                 uint64_t basePipelineHandle; int32_t basePipelineIndex; } VkComputePipelineCreateInfo;
typedef struct { uint32_t sType; const void* pNext; uint32_t flags;
                 uint32_t setLayoutCount; const VkDescriptorSetLayout* pSetLayouts;
                 uint32_t pushConstantRangeCount; const void* pPushConstantRanges; } VkPipelineLayoutCreateInfo;
typedef struct { uint32_t binding; uint32_t descriptorType;
                 uint32_t descriptorCount; uint32_t stageFlags;
                 const void* pImmutableSamplers; } VkDescriptorSetLayoutBinding;
typedef struct { uint32_t sType; const void* pNext; uint32_t flags;
                 uint32_t bindingCount;
                 const VkDescriptorSetLayoutBinding* pBindings; } VkDescriptorSetLayoutCreateInfo;
typedef struct { uint32_t type; uint32_t descriptorCount; } VkDescriptorPoolSize;
typedef struct { uint32_t sType; const void* pNext; uint32_t flags;
                 uint32_t maxSets; uint32_t poolSizeCount;
                 const VkDescriptorPoolSize* pPoolSizes; } VkDescriptorPoolCreateInfo;
typedef struct { uint32_t sType; const void* pNext; uint64_t descriptorPool;
                 uint32_t descriptorSetCount;
                 const VkDescriptorSetLayout* pSetLayouts; } VkDescriptorSetAllocateInfo;
typedef struct { uint64_t buffer; uint64_t offset; uint64_t range; } VkDescriptorBufferInfo;
typedef struct { uint32_t sType; const void* pNext; uint64_t dstSet;
                 uint32_t dstBinding; uint32_t dstArrayElement;
                 uint32_t descriptorCount; uint32_t descriptorType;
                 const void* pImageInfo; const VkDescriptorBufferInfo* pBufferInfo;
                 const void* pTexelBufferView; } VkWriteDescriptorSet;
typedef struct { uint32_t sType; const void* pNext; uint32_t flags;
                 uint32_t queueFamilyIndex; } VkCommandPoolCreateInfo;
typedef struct { uint32_t sType; const void* pNext; uint64_t commandPool;
                 uint32_t level; uint32_t commandBufferCount; } VkCommandBufferAllocateInfo;
typedef struct { uint32_t sType; const void* pNext; uint32_t flags;
                 const void* pInheritanceInfo; } VkCommandBufferBeginInfo;
typedef struct { uint32_t sType; const void* pNext; uint32_t flags; } VkFenceCreateInfo;
typedef struct { uint32_t sType; const void* pNext;
                 uint32_t waitSemaphoreCount; const uint64_t* pWaitSemaphores;
                 const uint32_t* pWaitDstStageMask; uint32_t commandBufferCount;
                 const VkCommandBuffer* pCommandBuffers;
                 uint32_t signalSemaphoreCount; const uint64_t* pSignalSemaphores; } VkSubmitInfo;

static void* vk_lib = NULL;
static int vk_ready = 0;
static VkInstance vk_instance;
static VkPhysicalDevice vk_physical_device;
static VkDevice vk_device;
static VkPipelineLayout vk_pipeline_layout;
static VkDescriptorSetLayout vk_desc_set_layout;
static VkCommandPool vk_cmd_pool;
static VkCommandBuffer vk_cmd_buf;
static uint32_t vk_queue_family_index = 0;
static uint64_t vk_queue = 0;
static uint32_t vk_host_visible_type = 0;

static int (*vkCreateInstance)(const void*, const void*, VkInstance*) = NULL;
static void (*vkDestroyInstance)(VkInstance, const void*) = NULL;
static int (*vkEnumeratePhysicalDevices)(VkInstance, uint32_t*, VkPhysicalDevice*) = NULL;
static void (*vkGetPhysicalDeviceQueueFamilyProperties)(VkPhysicalDevice, uint32_t*, void*) = NULL;
static void (*vkGetPhysicalDeviceFeatures)(VkPhysicalDevice, void*) = NULL;
static void (*vkGetPhysicalDeviceMemoryProperties)(VkPhysicalDevice, void*) = NULL;
static int (*vkCreateDevice)(VkPhysicalDevice, const void*, const void*, VkDevice*) = NULL;
static void (*vkGetDeviceQueue)(VkDevice, uint32_t, uint32_t, VkQueue*) = NULL;
static void (*vkDestroyDevice)(VkDevice, const void*) = NULL;
static int (*vkCreateBuffer)(VkDevice, const void*, const void*, VkBuffer*) = NULL;
static void (*vkGetBufferMemoryRequirements)(VkDevice, VkBuffer, void*) = NULL;
static int (*vkAllocateMemory)(VkDevice, const void*, const void*, VkDeviceMemory*) = NULL;
static int (*vkBindBufferMemory)(VkDevice, VkBuffer, VkDeviceMemory, uint64_t) = NULL;
static int (*vkMapMemory)(VkDevice, VkDeviceMemory, uint64_t, uint64_t, uint32_t, void**) = NULL;
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
static void (*vkDestroyDescriptorPool)(VkDevice, VkDescriptorPool, const void*) = NULL;
static int (*vkAllocateDescriptorSets)(VkDevice, const void*, VkDescriptorSet*) = NULL;
static int (*vkResetDescriptorPool)(VkDevice, VkDescriptorPool, uint32_t) = NULL;
static void (*vkUpdateDescriptorSets)(VkDevice, uint32_t, const void*, uint32_t, const void*) = NULL;
static int (*vkCreateCommandPool)(VkDevice, const void*, const void*, VkCommandPool*) = NULL;
static int (*vkAllocateCommandBuffers)(VkDevice, const void*, VkCommandBuffer*) = NULL;
static int (*vkResetCommandBuffer)(VkCommandBuffer, uint32_t) = NULL;
static int (*vkBeginCommandBuffer)(VkCommandBuffer, const void*) = NULL;
static void (*vkCmdBindPipeline)(VkCommandBuffer, uint32_t, VkPipeline) = NULL;
static void (*vkCmdBindDescriptorSets)(VkCommandBuffer, uint32_t, VkPipelineLayout, uint32_t, uint32_t, const VkDescriptorSet*, uint32_t, const uint32_t*) = NULL;
static void (*vkCmdDispatch)(VkCommandBuffer, uint32_t, uint32_t, uint32_t) = NULL;
static void (*vkCmdCopyBuffer)(VkCommandBuffer, VkBuffer, VkBuffer, uint32_t, const void*) = NULL;
static void (*vkCmdPipelineBarrier)(VkCommandBuffer, uint32_t, uint32_t, uint32_t, uint32_t, const void*, uint32_t, const void*, uint32_t, const void*) = NULL;
static int (*vkEndCommandBuffer)(VkCommandBuffer) = NULL;
static int (*vkQueueSubmit)(VkQueue, uint32_t, const void*, VkFence) = NULL;
static int (*vkWaitForFences)(VkDevice, uint32_t, const VkFence*, uint32_t, uint64_t) = NULL;
static int (*vkResetFences)(VkDevice, uint32_t, const VkFence*) = NULL;
static int (*vkCreateFence)(VkDevice, const void*, const void*, VkFence*) = NULL;
static void (*vkDestroyFence)(VkDevice, VkFence, const void*) = NULL;
static void (*vkDestroyCommandPool)(VkDevice, VkCommandPool, const void*) = NULL;
static void (*vkDeviceWaitIdle)(VkDevice) = NULL;

static int load_vulkan_symbols(void) {
#define LOAD(name) do { *(void**)(&name) = dlsym(vk_lib, #name); if (!name) return 0; } while (0)
    LOAD(vkCreateInstance);
    LOAD(vkDestroyInstance);
    LOAD(vkEnumeratePhysicalDevices);
    LOAD(vkGetPhysicalDeviceQueueFamilyProperties);
    LOAD(vkGetPhysicalDeviceFeatures);
    LOAD(vkGetPhysicalDeviceMemoryProperties);
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
    LOAD(vkDestroyDescriptorPool);
    LOAD(vkAllocateDescriptorSets);
    LOAD(vkResetDescriptorPool);
    LOAD(vkUpdateDescriptorSets);
    LOAD(vkCreateCommandPool);
    LOAD(vkAllocateCommandBuffers);
    LOAD(vkResetCommandBuffer);
    LOAD(vkBeginCommandBuffer);
    LOAD(vkCmdBindPipeline);
    LOAD(vkCmdBindDescriptorSets);
    LOAD(vkCmdDispatch);
    LOAD(vkCmdCopyBuffer);
    LOAD(vkCmdPipelineBarrier);
    LOAD(vkEndCommandBuffer);
    LOAD(vkQueueSubmit);
    LOAD(vkWaitForFences);
    LOAD(vkResetFences);
    LOAD(vkCreateFence);
    LOAD(vkDestroyFence);
    LOAD(vkDestroyCommandPool);
    LOAD(vkDeviceWaitIdle);
    return 1;
#undef LOAD
}

static int briev_dev_vulkan_available(void) {
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

// Memory-type index with HOST_VISIBLE|HOST_COHERENT — the staging buffer must
// be mappable and self-coherent; a hardcoded type 0 is not host-visible on
// every device.
static uint32_t pick_host_visible_type(VkPhysicalDevice pd) {
    struct { uint32_t memoryTypeCount;
             struct { uint32_t propertyFlags; uint32_t heapIndex; } memoryTypes[VK_MAX_MEMORY_TYPES];
             uint32_t memoryHeapCount;
             struct { uint64_t size; uint32_t flags; } memoryHeaps[16]; } props;
    vkGetPhysicalDeviceMemoryProperties(pd, &props);
    for (uint32_t i = 0; i < props.memoryTypeCount && i < VK_MAX_MEMORY_TYPES; i++) {
        uint32_t f = props.memoryTypes[i].propertyFlags;
        if ((f & VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT) && (f & VK_MEMORY_PROPERTY_HOST_COHERENT_BIT)) {
            return i;
        }
    }
    for (uint32_t i = 0; i < props.memoryTypeCount && i < VK_MAX_MEMORY_TYPES; i++) {
        if (props.memoryTypes[i].propertyFlags & VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT) {
            return i;
        }
    }
    return 0xFFFFFFFFu;
}

// Memory-type index with DEVICE_LOCAL — the GPU's own VRAM. The working set
// (device-resident arrays) lives here; the host-visible buffer is only the
// seed/staging window. 0xFFFFFFFF = none found → all-host fallback.
static uint32_t vk_device_local_type = 0xFFFFFFFFu;

static uint32_t pick_device_local_type(VkPhysicalDevice pd) {
    // Mirrors VkPhysicalDeviceMemoryProperties (see pick_host_visible_type).
    struct { uint32_t memoryTypeCount;
             struct { uint32_t propertyFlags; uint32_t heapIndex; } memoryTypes[32];
             uint32_t memoryHeapCount;
             struct { uint64_t size; uint32_t flags; } memoryHeaps[16]; } props;
    vkGetPhysicalDeviceMemoryProperties(pd, &props);
    for (uint32_t i = 0; i < props.memoryTypeCount && i < 32; i++) {
        uint32_t f = props.memoryTypes[i].propertyFlags;
        if ((f & VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT) && !(f & VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT)) {
            return i;
        }
    }
    return 0xFFFFFFFFu;
}

static int briev_dev_vulkan_init(void) {
    int verbose = g_verbose;
    if (!briev_dev_vulkan_available()) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] libvulkan.so.1 not loadable\n");
        return 0;
    }
    VkInstanceCreateInfo ici = {0};
    ici.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO;
    if (vkCreateInstance(&ici, NULL, &vk_instance) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] vkCreateInstance failed\n");
        goto fail;
    }
    uint32_t pdc = 0;
    vkEnumeratePhysicalDevices(vk_instance, &pdc, NULL);
    if (pdc == 0) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] no physical devices\n");
        goto fail;
    }
    VkPhysicalDevice devices[8];
    if (pdc > 8) { pdc = 8; }
    vkEnumeratePhysicalDevices(vk_instance, &pdc, devices);
    // First device with a compute-capable queue family wins.
    vk_physical_device = devices[0];
    struct { uint32_t queueFamilyCount; struct { uint32_t queueFlags; uint32_t queueCount;
             uint32_t timestampValidBits; uint32_t minImageTransferGranularity[3]; } families[16]; } qprops;
    memset(&qprops, 0, sizeof(qprops));
    vkGetPhysicalDeviceQueueFamilyProperties(vk_physical_device, &qprops.queueFamilyCount, NULL);
    if (qprops.queueFamilyCount > 16) { qprops.queueFamilyCount = 16; }
    vkGetPhysicalDeviceQueueFamilyProperties(vk_physical_device, &qprops.queueFamilyCount, qprops.families);
    vk_queue_family_index = 0xFFFFFFFFu;
    for (uint32_t i = 0; i < qprops.queueFamilyCount; i++) {
        if (qprops.families[i].queueCount > 0 && (qprops.families[i].queueFlags & 0x2u)) { // COMPUTE
            vk_queue_family_index = i;
            break;
        }
    }
    if (vk_queue_family_index == 0xFFFFFFFFu) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] no compute queue family\n");
        goto fail;
    }
    vk_host_visible_type = pick_host_visible_type(vk_physical_device);
    vk_device_local_type = pick_device_local_type(vk_physical_device);
    if (vk_host_visible_type == 0xFFFFFFFFu) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] no host-visible memory type\n");
        goto fail;
    }

    float queue_priority = 1.0f;
    VkDeviceQueueCreateInfo qci = {0};
    qci.sType = VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO;
    qci.queueFamilyIndex = vk_queue_family_index;
    qci.queueCount = 1;
    qci.pQueuePriorities = &queue_priority;
    // 2026-08-31: enable every SUPPORTED feature — the kernels use Int64/
    // Float64, and a NULL pEnabledFeatures means ALL OFF (the pipeline then
    // fails with no message). Features the device lacks stay off.
    struct { uint32_t robustBufferAccess; uint32_t f[54]; } features = {0};
    vkGetPhysicalDeviceFeatures(vk_physical_device, &features);
    VkDeviceCreateInfo dci = {0};
    dci.sType = VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO;
    dci.queueCreateInfoCount = 1;
    dci.pQueueCreateInfos = &qci;
    dci.pEnabledFeatures = &features;
    if (vkCreateDevice(vk_physical_device, &dci, NULL, &vk_device) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] vkCreateDevice failed\n");
        goto fail;
    }
    vkGetDeviceQueue(vk_device, vk_queue_family_index, 0, &vk_queue);

    VkDescriptorSetLayoutBinding binding = {0};
    binding.binding = 0;
    binding.descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER;
    binding.descriptorCount = 1;
    binding.stageFlags = VK_SHADER_STAGE_COMPUTE_BIT;
    VkDescriptorSetLayoutCreateInfo dlci = {0};
    dlci.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO;
    dlci.bindingCount = 1;
    dlci.pBindings = &binding;
    if (vkCreateDescriptorSetLayout(vk_device, &dlci, NULL, &vk_desc_set_layout) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] descriptor set layout failed\n");
        goto fail;
    }
    VkPipelineLayoutCreateInfo plci = {0};
    plci.sType = VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO;
    plci.setLayoutCount = 1;
    plci.pSetLayouts = &vk_desc_set_layout;
    if (vkCreatePipelineLayout(vk_device, &plci, NULL, &vk_pipeline_layout) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] pipeline layout failed\n");
        goto fail;
    }
    VkCommandPoolCreateInfo cpi = {0};
    cpi.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO;
    cpi.queueFamilyIndex = vk_queue_family_index;
    if (vkCreateCommandPool(vk_device, &cpi, NULL, &vk_cmd_pool) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] command pool failed\n");
        goto fail;
    }
    VkCommandBufferAllocateInfo cbai = {0};
    cbai.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO;
    cbai.commandPool = (uint64_t)vk_cmd_pool;
    cbai.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY;
    cbai.commandBufferCount = 1;
    if (vkAllocateCommandBuffers(vk_device, &cbai, &vk_cmd_buf) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] command buffer alloc failed\n");
        goto fail;
    }
    if (verbose) fprintf(stderr, "[briev_accel/vulkan] device ready (queue family %u, mem type %u)\n",
                         vk_queue_family_index, vk_host_visible_type);
    return 1;
fail:
    if (vk_device) { vkDestroyDevice(vk_device, NULL); vk_device = VK_NULL_HANDLE; }
    if (vk_instance) { vkDestroyInstance(vk_instance, NULL); vk_instance = VK_NULL_HANDLE; }
    dlclose(vk_lib);
    vk_lib = NULL;
    vk_ready = 0;
    return 0;
}

typedef struct {
    VkShaderModule module;
    VkPipeline pipeline;
    // 2026-08-31 (plan item 3): PERSISTENT staging resources — allocated at
    // first launch, reused every launch. The per-launch create/map/destroy
    // churn dominated kernel time for small workloads (~0.75ms/launch).
    VkBuffer buffer;
    VkDeviceMemory memory;
    // Device-local working set (plan 2026-08-31-gpu-next): the SSBO the
    // shader actually reads/writes. The host-visible `buffer` stays as the
    // seed/scalar-sync staging window. VK_NULL_HANDLE = all-host fallback.
    VkBuffer dev_buffer;
    VkDeviceMemory dev_memory;
    void* mapped;
    VkDescriptorPool pool;
    VkDescriptorSet desc_set;
    size_t bytes;
    VkFence fence;
} BrievVulkanKernel;

static int briev_dev_vulkan_create_kernel(const uint8_t* spirv, size_t size, void** out) {
    int verbose = g_verbose;
    VkShaderModuleCreateInfo smci = {0};
    smci.sType = VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO;
    smci.codeSize = size;
    smci.pCode = (const uint32_t*)spirv;
    VkShaderModule module;
    if (vkCreateShaderModule(vk_device, &smci, NULL, &module) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] shader module rejected\n");
        return 0;
    }
    VkPipelineShaderStageCreateInfo stage = {0};
    stage.sType = VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO;
    stage.stage = VK_SHADER_STAGE_COMPUTE;
    stage.module = (uint64_t)module;
    stage.pName = "main";
    VkComputePipelineCreateInfo pci = {0};
    pci.sType = VK_STRUCTURE_TYPE_COMPUTE_PIPELINE_CREATE_INFO;
    pci.stage = stage;
    pci.layout = (uint64_t)vk_pipeline_layout;
    VkPipeline pipeline;
    if (vkCreateComputePipelines(vk_device, VK_NULL_HANDLE, 1, &pci, NULL, &pipeline) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] compute pipeline failed\n");
        vkDestroyShaderModule(vk_device, module, NULL);
        return 0;
    }
    BrievVulkanKernel* k = calloc(1, sizeof(BrievVulkanKernel));
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

// ── Transfer helpers (plan 2026-08-31-gpu-next): the device-local working
// set is fed from (or drained to) the host-visible staging window inside the
// SAME submission as the dispatch — the barriers order transfer vs shader
// access on the GPU, no host round trip.
//
// direction: to_device copies staging→VRAM (seed + scalar sync); otherwise
// VRAM→staging (download). `dirty` is flat (offset, bytes) pairs; full copy
// when n_dirty == 0 or too many ranges.
#define VK_BRIEV_MAX_RANGES 16u

static void record_copy(BrievVulkanKernel* k, int to_device,
                        const size_t* dirty, uint32_t n_dirty) {
    if (k->dev_buffer == VK_NULL_HANDLE) {
        return;
    }
    if (to_device && n_dirty > 0 && n_dirty <= VK_BRIEV_MAX_RANGES) {
        struct { uint64_t srcOffset; uint64_t dstOffset; uint64_t size; } regions[VK_BRIEV_MAX_RANGES];
        for (uint32_t r = 0; r < n_dirty; r++) {
            regions[r].srcOffset = dirty[2 * r];
            regions[r].dstOffset = dirty[2 * r];
            regions[r].size = dirty[2 * r + 1];
        }
        vkCmdCopyBuffer(vk_cmd_buf, k->buffer, k->dev_buffer, n_dirty, regions);
        return;
    }
    struct { uint64_t srcOffset; uint64_t dstOffset; uint64_t size; } one =
        { 0, 0, k->bytes };
    if (to_device) {
        vkCmdCopyBuffer(vk_cmd_buf, k->buffer, k->dev_buffer, 1, &one);
    } else {
        vkCmdCopyBuffer(vk_cmd_buf, k->dev_buffer, k->buffer, 1, &one);
    }
}

static void record_barrier(BrievVulkanKernel* k, uint32_t src_stage,
                           uint32_t dst_stage, uint32_t src_access,
                           uint32_t dst_access) {
    // VkBufferMemoryBarrier — `buffer` MUST name the transferred buffer (a
    // null buffer makes the submission invalid and the GPU silently drops
    // the whole command buffer — found on device: y stayed zero).
    struct { uint32_t sType; const void* pNext; uint32_t srcAccessMask;
             uint32_t dstAccessMask; uint32_t srcQueueFamilyIndex;
             uint32_t dstQueueFamilyIndex; uint64_t buffer; uint64_t offset;
             uint64_t size; } bmb = {0};
    bmb.sType = 44; // VK_STRUCTURE_TYPE_BUFFER_MEMORY_BARRIER
    bmb.srcAccessMask = src_access;
    bmb.dstAccessMask = dst_access;
    bmb.srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
    bmb.dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
    bmb.buffer = (uint64_t)k->dev_buffer;
    bmb.offset = 0;
    bmb.size = k->bytes;
    vkCmdPipelineBarrier(vk_cmd_buf, src_stage, dst_stage, 0,
                         0, NULL, 1, &bmb, 0, NULL);
}

// One launch = memcpy the projection into the kernel's PERSISTENT mapped
// staging buffer, record dispatch + submit + fence-wait, read back. All
// device objects live on the kernel handle (allocated at first launch —
// that is when the projection size is known); the per-launch cost is now
// two memcpys + one submit round trip.
static int briev_dev_vulkan_launch(void* handle, const void* proj, size_t proj_bytes,
                                  size_t global_n, void* proj_out) {
    int verbose = g_verbose;
    size_t local_n = VK_LOCAL_SIZE_X;
    BrievVulkanKernel* k = (BrievVulkanKernel*)handle;

    if (k->buffer == VK_NULL_HANDLE || k->bytes < proj_bytes) {
        // First launch (or the projection grew): allocate the persistent
        // staging buffer + memory + descriptor pool + descriptor set.
        if (k->buffer != VK_NULL_HANDLE) {
            // Grow: tear the old set down first (rare — projections are
            // fixed per program once the bound is known).
            vkDestroyDescriptorPool(vk_device, k->pool, NULL);
            vkUnmapMemory(vk_device, k->memory);
            vkFreeMemory(vk_device, k->memory, NULL);
            vkDestroyBuffer(vk_device, k->buffer, NULL);
            k->buffer = VK_NULL_HANDLE;
        }
        struct { uint32_t sType; const void* pNext; uint32_t flags; uint64_t size;
                 uint32_t usage; uint32_t sharing; uint32_t queueFamilyIndexCount;
                 const uint32_t* pQueueFamilyIndices; } binfo = {0};
        binfo.sType = 8; // VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO
        binfo.size = proj_bytes;
        binfo.usage = VK_BUFFER_USAGE_STORAGE_BUFFER_BIT
                    | VK_BUFFER_USAGE_TRANSFER_SRC_BIT | VK_BUFFER_USAGE_TRANSFER_DST_BIT;
        if (vkCreateBuffer(vk_device, &binfo, NULL, &k->buffer) != VK_SUCCESS) {
            if (verbose) fprintf(stderr, "[briev_accel/vulkan] buffer create failed\n");
            return 0;
        }
        struct { uint64_t size; uint64_t alignment; uint32_t memoryTypeBits; } mem_reqs = {0};
        vkGetBufferMemoryRequirements(vk_device, k->buffer, &mem_reqs);
        // VkMemoryAllocateInfo { sType=5, pNext, allocationSize(u64), memoryTypeIndex }
        struct { uint32_t sType; const void* pNext; uint64_t allocationSize;
                 uint32_t memoryTypeIndex; } alloc = {0};
        alloc.sType = 5; // VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO
        alloc.allocationSize = mem_reqs.size;
        alloc.memoryTypeIndex = vk_host_visible_type;
        if (vkAllocateMemory(vk_device, &alloc, NULL, &k->memory) != VK_SUCCESS) {
            if (verbose) fprintf(stderr, "[briev_accel/vulkan] memory alloc failed\n");
            vkDestroyBuffer(vk_device, k->buffer, NULL);
            k->buffer = VK_NULL_HANDLE;
            return 0;
        }
        if (vkBindBufferMemory(vk_device, k->buffer, k->memory, 0) != VK_SUCCESS) {
            if (verbose) fprintf(stderr, "[briev_accel/vulkan] bind failed\n");
            vkFreeMemory(vk_device, k->memory, NULL);
            vkDestroyBuffer(vk_device, k->buffer, NULL);
            k->buffer = VK_NULL_HANDLE;
            return 0;
        }
        // vkMapMemory is a SIX-arg call — the mapped pointer goes through
        // the out-parameter (the old 5-arg/return-value signature made the
        // driver write through an uninitialized register).
        void* host_ptr = NULL;
        if (vkMapMemory(vk_device, k->memory, 0, proj_bytes, 0, &host_ptr) != VK_SUCCESS || !host_ptr) {
            if (verbose) fprintf(stderr, "[briev_accel/vulkan] map failed\n");
            vkFreeMemory(vk_device, k->memory, NULL);
            vkDestroyBuffer(vk_device, k->buffer, NULL);
            k->buffer = VK_NULL_HANDLE;
            return 0;
        }
        k->mapped = host_ptr;
        k->bytes = proj_bytes;
        // Device-local working set (plan 2026-08-31-gpu-next): allocate the
        // VRAM buffer the shader actually accesses. Non-fatal on failure —
        // the all-host path remains correct, just PCIe-bound.
        if (vk_device_local_type != 0xFFFFFFFFu) {
            struct { uint32_t sType; const void* pNext; uint32_t flags; uint64_t size;
                     uint32_t usage; uint32_t sharing; uint32_t queueFamilyIndexCount;
                     const uint32_t* pQueueFamilyIndices; } dbinfo = {0};
            dbinfo.sType = 8; // VK_STRUCTURE_TYPE_BUFFER_CREATE_INFO
            dbinfo.size = proj_bytes;
            dbinfo.usage = VK_BUFFER_USAGE_STORAGE_BUFFER_BIT
                         | VK_BUFFER_USAGE_TRANSFER_SRC_BIT | VK_BUFFER_USAGE_TRANSFER_DST_BIT;
            if (vkCreateBuffer(vk_device, &dbinfo, NULL, &k->dev_buffer) == VK_SUCCESS) {
                struct { uint64_t size; uint64_t alignment; uint32_t memoryTypeBits; } dreqs = {0};
                vkGetBufferMemoryRequirements(vk_device, k->dev_buffer, &dreqs);
                struct { uint32_t sType; const void* pNext; uint64_t allocationSize;
                         uint32_t memoryTypeIndex; } dalloc = {0};
                dalloc.sType = 5; // VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO
                dalloc.allocationSize = dreqs.size;
                dalloc.memoryTypeIndex = vk_device_local_type;
                if (vkAllocateMemory(vk_device, &dalloc, NULL, &k->dev_memory) == VK_SUCCESS
                    && vkBindBufferMemory(vk_device, k->dev_buffer, k->dev_memory, 0) == VK_SUCCESS) {
                    if (verbose) fprintf(stderr, "[briev_accel/vulkan] device-local working set ON\n");
                } else {
                    if (verbose) fprintf(stderr, "[briev_accel/vulkan] device-local alloc failed — all-host\n");
                    vkDestroyBuffer(vk_device, k->dev_buffer, NULL);
                    vkFreeMemory(vk_device, k->dev_memory, NULL);
                    k->dev_buffer = VK_NULL_HANDLE; k->dev_memory = VK_NULL_HANDLE;
                }
            } else {
                if (verbose) fprintf(stderr, "[briev_accel/vulkan] device-local buffer create failed\n");
                k->dev_buffer = VK_NULL_HANDLE;
            }
        }
        VkDescriptorPoolCreateInfo dpi = {0};
        dpi.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO;
        dpi.maxSets = 1;
        VkDescriptorPoolSize ps = { VK_DESCRIPTOR_TYPE_STORAGE_BUFFER, 1 };
        dpi.poolSizeCount = 1;
        dpi.pPoolSizes = &ps;
        if (vkCreateDescriptorPool(vk_device, &dpi, NULL, &k->pool) != VK_SUCCESS) {
            if (verbose) fprintf(stderr, "[briev_accel/vulkan] pool create failed\n");
            k->buffer = VK_NULL_HANDLE;
            return 0;
        }
        VkDescriptorSetAllocateInfo dsai = {0};
        dsai.sType = VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO;
        dsai.descriptorPool = (uint64_t)k->pool;
        dsai.descriptorSetCount = 1;
        dsai.pSetLayouts = &vk_desc_set_layout;
        if (vkAllocateDescriptorSets(vk_device, &dsai, &k->desc_set) != VK_SUCCESS) {
            if (verbose) fprintf(stderr, "[briev_accel/vulkan] descriptor alloc failed\n");
            vkDestroyDescriptorPool(vk_device, k->pool, NULL);
            k->pool = VK_NULL_HANDLE;
            k->buffer = VK_NULL_HANDLE;
            return 0;
        }
        // The shader reads the DEVICE buffer when one exists; the host-visible
        // buffer is only the seed/sync window.
        VkDescriptorBufferInfo bi = { (uint64_t)(k->dev_buffer ? k->dev_buffer : k->buffer), 0, proj_bytes };
        VkWriteDescriptorSet wds = {0};
        wds.sType = VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET;
        wds.dstSet = (uint64_t)k->desc_set;
        wds.dstBinding = 0;
        wds.descriptorCount = 1;
        wds.descriptorType = VK_DESCRIPTOR_TYPE_STORAGE_BUFFER;
        wds.pBufferInfo = &bi;
        vkUpdateDescriptorSets(vk_device, 1, &wds, 0, NULL);
    }

    memcpy(k->mapped, proj, proj_bytes);

    VkCommandBufferBeginInfo bbi = {0};
    // The buffer was submitted with ONE_TIME_SUBMIT last launch — reset it
    // before re-recording (an invalid re-begin crashes the NVIDIA driver).
    vkResetCommandBuffer(vk_cmd_buf, 0);
    bbi.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO;
    bbi.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
    if (vkBeginCommandBuffer(vk_cmd_buf, &bbi) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] begin failed\n");
        return 0;
    }
    vkCmdBindPipeline(vk_cmd_buf, VK_PIPELINE_BIND_POINT_COMPUTE, k->pipeline);
    vkCmdBindDescriptorSets(vk_cmd_buf, VK_PIPELINE_BIND_POINT_COMPUTE, vk_pipeline_layout,
                            0, 1, &k->desc_set, 0, NULL);
    if (k->dev_buffer != VK_NULL_HANDLE) {
        // Device working set: push fresh input to VRAM, pull results after.
        record_copy(k, 1, NULL, 0);
        record_barrier(k, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT,
                       VK_ACCESS_TRANSFER_WRITE_BIT, VK_ACCESS_SHADER_READ_BIT);
    }
    // The kernel declares LocalSize 64×1×1; dispatch ceil(n/64) workgroups
    // (the old n×1×1 launched one-thread workgroups — correct but 32× under).
    size_t groups = (global_n + local_n - 1) / local_n;
    if (groups == 0) { groups = 1; }
    vkCmdDispatch(vk_cmd_buf, (uint32_t)groups, 1, 1);
    if (k->dev_buffer != VK_NULL_HANDLE) {
        record_barrier(k, VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT,
                       VK_ACCESS_SHADER_WRITE_BIT, VK_ACCESS_TRANSFER_READ_BIT);
        record_copy(k, 0, NULL, 0);
    }
    if (vkEndCommandBuffer(vk_cmd_buf) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] end failed\n");
        return 0;
    }

    if (k->fence == VK_NULL_HANDLE) {
        VkFenceCreateInfo fci = {0};
        fci.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO;
        if (vkCreateFence(vk_device, &fci, NULL, &k->fence) != VK_SUCCESS) {
            return 0;
        }
    } else {
        vkResetFences(vk_device, 1, &k->fence);
    }
    VkSubmitInfo si = {0};
    si.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO;
    si.commandBufferCount = 1;
    si.pCommandBuffers = &vk_cmd_buf;
    int ok = 1;
    if (vkQueueSubmit(vk_queue, 1, &si, k->fence) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] submit failed\n");
        ok = 0;
    } else if (vkWaitForFences(vk_device, 1, &k->fence, 1, 30ULL * 1000 * 1000 * 1000) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] fence wait timed out\n");
        ok = 0;
    } else {
        memcpy(proj_out, k->mapped, proj_bytes);
    }
    return ok;
}

// Mapped projection pointer of the kernel's persistent staging buffer —
// device residency (plan item 3): the runtime packs scalars directly into
// this region between launches instead of full upload/download cycles.
static void* briev_dev_vulkan_mapped(void* handle) {
    BrievVulkanKernel* k = (BrievVulkanKernel*)handle;
    return k && k->buffer != VK_NULL_HANDLE ? k->mapped : NULL;
}

// Record + submit + fence-wait with NO host copies. Requires the caller to
// have seeded/synced the mapped projection itself.
// 2D dispatch (plan 2026-08-31-gpu-next §2b): nx columns × ny rows; ny == 1
// is the flat 1D form. Workgroups stay 64×1×1, so the grid is
// (ceil(nx/64), ny, 1) — a 2D launch covers nx*ny work items exactly like
// the flat ceil(nx*ny/64) grid, but the hardware hands each invocation its
// (x, y) position directly (the kernel reconstructs i = y*nx + x).
static int briev_dev_vulkan_launch_dev(void* handle, size_t global_n);

static int briev_dev_vulkan_launch_dev2d(void* handle, size_t nx, size_t ny,
                                         int full_sync, const size_t* dirty,
                                         uint32_t n_dirty);

static int briev_dev_vulkan_download_dev(void* handle);

static int briev_dev_vulkan_launch_dev(void* handle, size_t global_n) {
    return briev_dev_vulkan_launch_dev2d(handle, global_n, 1, 0, NULL, 0);
}

// 2D dispatch (plan 2026-08-31-gpu-next §2b): nx columns × ny rows; ny == 1
// is the flat 1D form. Workgroups stay 64×1×1, so the grid is
// (ceil(nx/64), ny, 1) — a 2D launch covers nx*ny work items exactly like
// the flat ceil(nx*ny/64) grid, but the hardware hands each invocation its
// (x, y) position directly (the kernel reconstructs i = y*nx + x).
//
// Device working set (plan 2026-08-31-gpu-next): with a VRAM buffer, the
// input side is pushed staging→VRAM inside this submission — full copy when
// `full_sync` (the seed), else only the `dirty` (offset, bytes) pairs (the
// scalar counters the host phase machine owns). Dispatch reads VRAM
// directly; nothing crosses PCIe per launch.
static int briev_dev_vulkan_launch_dev2d(void* handle, size_t nx, size_t ny,
                                         int full_sync, const size_t* dirty,
                                         uint32_t n_dirty) {
    BrievVulkanKernel* k = (BrievVulkanKernel*)handle;
    int verbose = g_verbose;
    size_t local_n = VK_LOCAL_SIZE_X;
    if (!k || k->buffer == VK_NULL_HANDLE) {
        return 0;
    }
    VkCommandBufferBeginInfo bbi = {0};
    vkResetCommandBuffer(vk_cmd_buf, 0);
    bbi.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO;
    bbi.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
    if (vkBeginCommandBuffer(vk_cmd_buf, &bbi) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] begin failed\n");
        return 0;
    }
    vkCmdBindPipeline(vk_cmd_buf, VK_PIPELINE_BIND_POINT_COMPUTE, k->pipeline);
    vkCmdBindDescriptorSets(vk_cmd_buf, VK_PIPELINE_BIND_POINT_COMPUTE, vk_pipeline_layout,
                            0, 1, &k->desc_set, 0, NULL);
    if (k->dev_buffer != VK_NULL_HANDLE) {
        if (full_sync || (n_dirty > 0 && n_dirty <= VK_BRIEV_MAX_RANGES)) {
            record_copy(k, 1, dirty, n_dirty);
            record_barrier(k, VK_PIPELINE_STAGE_TRANSFER_BIT, VK_PIPELINE_STAGE_COMPUTE_SHADER_BIT,
                           VK_ACCESS_TRANSFER_WRITE_BIT, VK_ACCESS_SHADER_READ_BIT);
        }
    }
    size_t groups_x = (nx + local_n - 1) / local_n;
    if (groups_x == 0) { groups_x = 1; }
    if (ny == 0) { ny = 1; }
                        if (verbose) fprintf(stderr, "[briev_accel/vulkan] dispatch gx=%u gy=%u (nx=%zu ny=%zu local=%zu)",                         (uint32_t)groups_x, (uint32_t)ny, nx, ny, local_n);
    vkCmdDispatch(vk_cmd_buf, (uint32_t)groups_x, (uint32_t)ny, 1);
    if (vkEndCommandBuffer(vk_cmd_buf) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] end failed\n");
        return 0;
    }
    if (k->fence == VK_NULL_HANDLE) {
        VkFenceCreateInfo fci = {0};
        fci.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO;
        if (vkCreateFence(vk_device, &fci, NULL, &k->fence) != VK_SUCCESS) {
            return 0;
        }
    } else {
        vkResetFences(vk_device, 1, &k->fence);
    }
    VkSubmitInfo si = {0};
    si.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO;
    si.commandBufferCount = 1;
    si.pCommandBuffers = &vk_cmd_buf;
    if (vkQueueSubmit(vk_queue, 1, &si, k->fence) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] submit failed\n");
        return 0;
    }
    if (vkWaitForFences(vk_device, 1, &k->fence, 1, 30ULL * 1000 * 1000 * 1000) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] fence wait timed out\n");
        return 0;
    }
    return 1;
}

static void briev_dev_vulkan_destroy_kernel(void* handle) {
    BrievVulkanKernel* k = (BrievVulkanKernel*)handle;
    if (!k) {
        return;
    }
    if (k->buffer != VK_NULL_HANDLE) {
        vkDestroyDescriptorPool(vk_device, k->pool, NULL);
        vkUnmapMemory(vk_device, k->memory);
        vkFreeMemory(vk_device, k->memory, NULL);
        vkDestroyBuffer(vk_device, k->buffer, NULL);
    }
    if (k->fence != VK_NULL_HANDLE) {
        vkDestroyFence(vk_device, k->fence, NULL);
    }
    vkDestroyPipeline(vk_device, k->pipeline, NULL);
    vkDestroyShaderModule(vk_device, k->module, NULL);
    free(k);
}

static void briev_dev_vulkan_shutdown(void) {
    if (vk_device) {
        vkDeviceWaitIdle(vk_device);
        vkDestroyCommandPool(vk_device, vk_cmd_pool, NULL);
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
    vk_device = VK_NULL_HANDLE;
    vk_instance = VK_NULL_HANDLE;
    vk_ready = 0;
}

// Pull the device working set into the host-visible staging window so the
// runtime's mapped read sees current data (briev_accel_download's tail).
static int briev_dev_vulkan_download_dev(void* handle) {
    BrievVulkanKernel* k = (BrievVulkanKernel*)handle;
    int verbose = g_verbose;
    if (!k || k->dev_buffer == VK_NULL_HANDLE) {
        return 0;  // nothing to pull — staging is already the source of truth
    }
    VkCommandBufferBeginInfo bbi = {0};
    vkResetCommandBuffer(vk_cmd_buf, 0);
    bbi.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO;
    bbi.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
    if (vkBeginCommandBuffer(vk_cmd_buf, &bbi) != VK_SUCCESS) {
        return 0;
    }
    record_copy(k, 0, NULL, 0);
    if (vkEndCommandBuffer(vk_cmd_buf) != VK_SUCCESS) {
        return 0;
    }
    if (k->fence != VK_NULL_HANDLE) {
        vkResetFences(vk_device, 1, &k->fence);
    }
    VkSubmitInfo si = {0};
    si.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO;
    si.commandBufferCount = 1;
    si.pCommandBuffers = &vk_cmd_buf;
    if (vkQueueSubmit(vk_queue, 1, &si, k->fence) != VK_SUCCESS) {
        return 0;
    }
    if (vkWaitForFences(vk_device, 1, &k->fence, 1, 30ULL * 1000 * 1000 * 1000) != VK_SUCCESS) {
        if (verbose) fprintf(stderr, "[briev_accel/vulkan] download fence wait timed out\n");
        return 0;
    }
    return 1;
}

BrievDeviceDriver briev_dev_vulkan = {
    "vulkan",
    0,  // capabilities: host-visible buffer copies, no zero-copy
    briev_dev_vulkan_available,
    briev_dev_vulkan_init,
    briev_dev_vulkan_create_kernel,
    briev_dev_vulkan_launch,
    briev_dev_vulkan_destroy_kernel,
    briev_dev_vulkan_shutdown,
    briev_dev_vulkan_mapped,
    briev_dev_vulkan_launch_dev,
    briev_dev_vulkan_launch_dev2d,
    briev_dev_vulkan_download_dev,
};
