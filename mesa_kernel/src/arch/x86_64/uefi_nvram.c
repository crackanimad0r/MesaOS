/**
 * UEFI NVRAM Implementation for MesaOS Kernel
 * 
 * Implementation of UEFI Runtime Services variable operations.
 * This file contains the actual function implementations.
 * 
 * No external libraries, no stdlib, pure C for x86_64 UEFI environment.
 */

#include "uefi_nvram.h"

/* ============================================================
 * Global Runtime Services Pointer
 * ============================================================ */

EFI_RUNTIME_SERVICES* gRT = NULL;

/* ============================================================
 * Initialization
 * ============================================================ */

void uefi_nvram_init(EFI_RUNTIME_SERVICES* runtime_services)
{
    gRT = runtime_services;
}

/* ============================================================
 * Set Variable - Store data in NVRAM
 * ============================================================ */

EFI_STATUS uefi_set_var(
    IN CHAR16* name,
    IN EFI_GUID* guid,
    IN VOID* data,
    IN UINTN data_size
)
{
    if (gRT == NULL || gRT->SetVariable == NULL) {
        return EFI_NOT_READY;
    }
    
    if (name == NULL || guid == NULL) {
        return EFI_INVALID_PARAMETER;
    }
    
    if (data == NULL && data_size > 0) {
        return EFI_INVALID_PARAMETER;
    }
    
    /* Use standard attributes for persistent data:
     * - NON_VOLATILE: Data survives power cycles (written to flash/NVRAM)
     * - BOOTSERVICE_ACCESS: Available during boot services phase
     * - RUNTIME_ACCESS: Available after ExitBootServices() at runtime
     * 
     * This combination (0x00000007) is the standard for kernel data that must
     * persist across reboots and be accessible at runtime.
     */
    UINT32 attributes = EFI_VARIABLE_STANDARD_ATTRIBUTES;
    
    return gRT->SetVariable(name, guid, attributes, data_size, data);
}

/* ============================================================
 * Get Variable - Read data from NVRAM (simple version)
 * ============================================================ */

EFI_STATUS uefi_get_var(
    IN CHAR16* name,
    IN EFI_GUID* guid,
    OUT VOID* buffer,
    IN OUT UINTN* buffer_size
)
{
    UINT32 attributes;
    return uefi_get_var_ex(name, guid, &attributes, buffer, buffer_size);
}

/* ============================================================
 * Get Variable Extended - Read data with attributes
 * ============================================================ */

EFI_STATUS uefi_get_var_ex(
    IN CHAR16* name,
    IN EFI_GUID* guid,
    OUT UINT32* attributes,
    OUT VOID* buffer,
    IN OUT UINTN* buffer_size
)
{
    if (gRT == NULL || gRT->GetVariable == NULL) {
        return EFI_NOT_READY;
    }
    
    if (name == NULL || guid == NULL || buffer_size == NULL) {
        return EFI_INVALID_PARAMETER;
    }
    
    if (attributes == NULL) {
        return EFI_INVALID_PARAMETER;
    }
    
    /* Call UEFI GetVariable:
     * - If buffer is NULL or *buffer_size is too small, returns EFI_BUFFER_TOO_SMALL
     *   and updates *buffer_size with the required size.
     * - On success, returns EFI_SUCCESS and updates *buffer_size with actual data size.
     * - attributes receives the variable's attributes (NON_VOLATILE, etc.)
     */
    return gRT->GetVariable(name, guid, attributes, buffer_size, buffer);
}

/* ============================================================
 * Delete Variable - Remove from NVRAM
 * ============================================================ */

EFI_STATUS uefi_delete_var(
    IN CHAR16* name,
    IN EFI_GUID* guid
)
{
    if (gRT == NULL || gRT->SetVariable == NULL) {
        return EFI_NOT_READY;
    }
    
    if (name == NULL || guid == NULL) {
        return EFI_INVALID_PARAMETER;
    }
    
    /* To delete a variable, call SetVariable with:
     * - DataSize = 0
     * - Data = NULL
     * - Attributes = 0 (or any value, ignored when DataSize=0)
     */
    return gRT->SetVariable(name, guid, 0, 0, NULL);
}

/* ============================================================
 * Enumerate Variables (placeholder - requires heap allocation)
 * ============================================================ */

EFI_STATUS uefi_enum_vars(
    IN EFI_GUID* guid,
    IN UEFI_VAR_ENUM_CALLBACK callback,
    IN VOID* context
)
{
    EFI_STATUS status;
    UINTN name_size = 0;
    EFI_GUID vendor_guid;
    
    (void)context; /* Unused in placeholder implementation */
    
    if (gRT == NULL || gRT->GetNextVariableName == NULL) {
        return EFI_NOT_READY;
    }
    
    if (callback == NULL) {
        return EFI_INVALID_PARAMETER;
    }
    
    /* First call: get required buffer size */
    status = gRT->GetNextVariableName(&name_size, NULL, guid ? &vendor_guid : NULL);
    if (status != EFI_BUFFER_TOO_SMALL) {
        return status;
    }
    
    /* Enumeration requires memory allocation for name_buffer.
     * This is a placeholder - kernel must provide an allocator. */
    
    return EFI_UNSUPPORTED;
}