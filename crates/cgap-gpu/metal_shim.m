/*
 * Freestanding Metal Unified Memory Bridge for cgap.
 *
 * Exposes MTLDevice zero-copy buffer wrapping over existing 64-byte aligned pointers
 * using MTLResourceStorageModeShared.
 */

#import <Metal/Metal.h>

void* cgap_metal_create_shared_buffer(void* bytes, size_t length) {
    if (!bytes || length == 0) {
        return NULL;
    }

    id<MTLDevice> device = MTLCreateSystemDefaultDevice();
    if (!device) {
        return NULL;
    }

    // Wrap CPU pointer directly into GPU unified memory with zero bus copies
    id<MTLBuffer> buffer = [device newBufferWithBytesNoCopy:bytes
                                                     length:length
                                                    options:MTLResourceStorageModeShared
                                                deallocator:nil];
    return (__bridge_retained void*)buffer;
}

void cgap_metal_release_buffer(void* buffer_ref) {
    if (buffer_ref) {
        id<MTLBuffer> buffer = (__bridge_transfer id<MTLBuffer>)buffer_ref;
        (void)buffer; // Let ARC release it
    }
}
