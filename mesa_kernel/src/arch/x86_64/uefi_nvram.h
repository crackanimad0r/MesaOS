/**
 * UEFI NVRAM Header for MesaOS Kernel
 * 
 * This module provides bare-metal access to UEFI Runtime Services
 * for persistent variable storage (NVRAM).
 * 
 * No external libraries, no stdlib, pure C for x86_64 UEFI environment.
 */

#ifndef UEFI_NVRAM_H
#define UEFI_NVRAM_H

/* UEFI Calling Convention Macros (parameter direction attributes) */
#ifndef IN
#define IN
#endif
#ifndef OUT
#define OUT
#endif
#ifndef OPTIONAL
#define OPTIONAL
#endif
#ifndef NULL
#define NULL ((void*)0)
#endif

/* ============================================================
 * Basic UEFI Type Definitions
 * ============================================================ */

typedef unsigned long long  UINT64;
typedef long long           INT64;
typedef unsigned int        UINT32;
typedef int                 INT32;
typedef unsigned short      UINT16;
typedef short               INT16;
typedef unsigned char       UINT8;
typedef char                INT8;
typedef unsigned long long  UINTN;
typedef long long           INTN;
typedef char                BOOLEAN;
typedef void*               VOID;
typedef UINT16              CHAR16;

/* EFI Status Codes */
typedef UINTN EFI_STATUS;

#define EFI_SUCCESS                 0
#define EFI_LOAD_ERROR              (1 | (1ULL << 63))
#define EFI_INVALID_PARAMETER       (2 | (1ULL << 63))
#define EFI_UNSUPPORTED             (3 | (1ULL << 63))
#define EFI_BAD_BUFFER_SIZE         (4 | (1ULL << 63))
#define EFI_BUFFER_TOO_SMALL        (5 | (1ULL << 63))
#define EFI_NOT_READY               (6 | (1ULL << 63))
#define EFI_DEVICE_ERROR            (7 | (1ULL << 63))
#define EFI_WRITE_PROTECTED         (8 | (1ULL << 63))
#define EFI_OUT_OF_RESOURCES        (9 | (1ULL << 63))
#define EFI_VOLUME_CORRUPTED        (10 | (1ULL << 63))
#define EFI_VOLUME_FULL             (11 | (1ULL << 63))
#define EFI_NO_MEDIA                (12 | (1ULL << 63))
#define EFI_MEDIA_CHANGED           (13 | (1ULL << 63))
#define EFI_NOT_FOUND               (14 | (1ULL << 63))
#define EFI_ACCESS_DENIED           (15 | (1ULL << 63))
#define EFI_NO_RESPONSE             (16 | (1ULL << 63))
#define EFI_NO_MAPPING              (17 | (1ULL << 63))
#define EFI_TIMEOUT                 (18 | (1ULL << 63))
#define EFI_NOT_STARTED             (19 | (1ULL << 63))
#define EFI_ALREADY_STARTED         (20 | (1ULL << 63))
#define EFI_ABORTED                 (21 | (1ULL << 63))
#define EFI_ICMP_ERROR              (22 | (1ULL << 63))
#define EFI_TFTP_ERROR              (23 | (1ULL << 63))
#define EFI_PROTOCOL_ERROR          (24 | (1ULL << 63))
#define EFI_INCOMPATIBLE_VERSION    (25 | (1ULL << 63))
#define EFI_SECURITY_VIOLATION      (26 | (1ULL << 63))
#define EFI_CRC_ERROR               (27 | (1ULL << 63))
#define EFI_END_OF_MEDIA            (28 | (1ULL << 63))
#define EFI_END_OF_FILE             (31 | (1ULL << 63))
#define EFI_INVALID_LANGUAGE        (32 | (1ULL << 63))
#define EFI_COMPROMISED_DATA        (33 | (1ULL << 63))
#define EFI_IP_ADDRESS_CONFLICT     (34 | (1ULL << 63))
#define EFI_HTTP_ERROR              (35 | (1ULL << 63))

#define EFI_ERROR(status)           (((INTN)(status)) < 0)

/* ============================================================
 * EFI_GUID Definition
 * ============================================================ */

typedef struct {
    UINT32  Data1;
    UINT16  Data2;
    UINT16  Data3;
    UINT8   Data4[8];
} EFI_GUID;

/* ============================================================
 * EFI_TIME Definition (for GetVariable/SetVariable)
 * ============================================================ */

typedef struct {
    UINT16  Year;
    UINT8   Month;
    UINT8   Day;
    UINT8   Hour;
    UINT8   Minute;
    UINT8   Second;
    UINT8   Pad1;
    UINT32  Nanosecond;
    INT16   TimeZone;
    UINT8   Daylight;
    UINT8   Pad2;
} EFI_TIME;

/* ============================================================
 * Variable Attributes
 * ============================================================ */

#define EFI_VARIABLE_NON_VOLATILE                       0x00000001
#define EFI_VARIABLE_BOOTSERVICE_ACCESS                 0x00000002
#define EFI_VARIABLE_RUNTIME_ACCESS                     0x00000004
#define EFI_VARIABLE_HARDWARE_ERROR_RECORD              0x00000008
#define EFI_VARIABLE_AUTHENTICATED_WRITE_ACCESS         0x00000010
#define EFI_VARIABLE_TIME_BASED_AUTHENTICATED_WRITE_ACCESS  0x00000020
#define EFI_VARIABLE_APPEND_WRITE                       0x00000040

/* Standard attribute combination for persistent data:
 * - NON_VOLATILE: Survives power cycles (stored in NVRAM)
 * - BOOTSERVICE_ACCESS: Accessible during boot services
 * - RUNTIME_ACCESS: Accessible after ExitBootServices()
 * 
 * This combination (0x00000007) ensures data persists across reboots
 * and is accessible both during boot and at runtime.
 */
#define EFI_VARIABLE_STANDARD_ATTRIBUTES  \
    (EFI_VARIABLE_NON_VOLATILE | EFI_VARIABLE_BOOTSERVICE_ACCESS | EFI_VARIABLE_RUNTIME_ACCESS)

/* ============================================================
 * EFI_RUNTIME_SERVICES Function Pointer Types
 * ============================================================ */

typedef EFI_STATUS (*EFI_GET_TIME)(
    OUT EFI_TIME* Time,
    OUT UINTN* Capabilities
);

typedef EFI_STATUS (*EFI_SET_TIME)(
    IN EFI_TIME* Time
);

typedef EFI_STATUS (*EFI_GET_WAKEUP_TIME)(
    OUT BOOLEAN* Enabled,
    OUT BOOLEAN* Pending,
    OUT EFI_TIME* Time
);

typedef EFI_STATUS (*EFI_SET_WAKEUP_TIME)(
    IN BOOLEAN Enabled,
    IN EFI_TIME* Time
);

/* SetVariable Function Prototype */
typedef EFI_STATUS (*EFI_SET_VARIABLE)(
    IN CHAR16* VariableName,
    IN EFI_GUID* VendorGuid,
    IN UINT32 Attributes,
    IN UINTN DataSize,
    IN VOID* Data
);

/* GetVariable Function Prototype */
typedef EFI_STATUS (*EFI_GET_VARIABLE)(
    IN CHAR16* VariableName,
    IN EFI_GUID* VendorGuid,
    OUT UINT32* Attributes,
    OUT UINTN* DataSize,
    OUT VOID* Data
);

/* GetNextVariableName Function Prototype */
typedef EFI_STATUS (*EFI_GET_NEXT_VARIABLE_NAME)(
    IN OUT UINTN* VariableNameSize,
    IN OUT CHAR16* VariableName,
    IN OUT EFI_GUID* VendorGuid
);

/* ============================================================
 * EFI_RUNTIME_SERVICES Structure (Partial - only what we need)
 * ============================================================ */

typedef struct {
    /* Time Services */
    EFI_GET_TIME            GetTime;
    EFI_SET_TIME            SetTime;
    EFI_GET_WAKEUP_TIME     GetWakeupTime;
    EFI_SET_WAKEUP_TIME     SetWakeupTime;

    /* Virtual Memory Services (not used in this implementation) */
    VOID*                   SetVirtualAddressMap;
    VOID*                   ConvertPointer;

    /* Variable Services */
    EFI_GET_VARIABLE        GetVariable;
    EFI_GET_NEXT_VARIABLE_NAME GetNextVariableName;
    EFI_SET_VARIABLE        SetVariable;

    /* Miscellaneous Services */
    VOID*                   GetNextHighMonotonicCount;
    VOID*                   ResetSystem;

    /* UEFI 2.0+ Capsule Services */
    VOID*                   UpdateCapsule;
    VOID*                   QueryCapsuleCapabilities;

    /* UEFI 2.0+ Misc */
    VOID*                   QueryVariableInfo;

} EFI_RUNTIME_SERVICES;

/* ============================================================
 * Global Runtime Services Pointer (to be set during boot)
 * ============================================================ */

extern EFI_RUNTIME_SERVICES* gRT;

/* ============================================================
 * Function Prototypes
 * ============================================================ */

/**
 * Initialize UEFI NVRAM access
 * Call this after ExitBootServices() with the RuntimeServices pointer
 * from EFI_SYSTEM_TABLE->RuntimeServices
 * 
 * @param runtime_services  Pointer to EFI_RUNTIME_SERVICES from EFI_SYSTEM_TABLE
 */
void uefi_nvram_init(EFI_RUNTIME_SERVICES* runtime_services);

/**
 * Set a UEFI Variable (persist data to NVRAM)
 * 
 * @param name        Null-terminated UTF-16 variable name
 * @param guid        Pointer to EFI_GUID identifying the variable namespace
 * @param data        Pointer to data buffer
 * @param data_size   Size of data in bytes
 * @return            EFI_STATUS result
 * 
 * Attributes used: EFI_VARIABLE_NON_VOLATILE | EFI_VARIABLE_BOOTSERVICE_ACCESS | EFI_VARIABLE_RUNTIME_ACCESS
 * This ensures the variable:
 *   - Persists across power cycles (NON_VOLATILE)
 *   - Is accessible during boot services (BOOTSERVICE_ACCESS)
 *   - Is accessible at runtime after ExitBootServices() (RUNTIME_ACCESS)
 */
EFI_STATUS uefi_set_var(
    IN CHAR16* name,
    IN EFI_GUID* guid,
    IN VOID* data,
    IN UINTN data_size
);

/**
 * Get a UEFI Variable (read data from NVRAM)
 * 
 * @param name        Null-terminated UTF-16 variable name
 * @param guid        Pointer to EFI_GUID identifying the variable namespace
 * @param buffer      Pointer to output buffer (can be NULL to query size)
 * @param buffer_size IN: size of buffer / OUT: required size or actual data size
 * @return            EFI_STATUS result
 * 
 * If buffer is NULL or *buffer_size is too small, returns EFI_BUFFER_TOO_SMALL
 * and updates *buffer_size with required size.
 */
EFI_STATUS uefi_get_var(
    IN CHAR16* name,
    IN EFI_GUID* guid,
    OUT VOID* buffer,
    IN OUT UINTN* buffer_size
);

/**
 * Get a UEFI Variable with Attributes (extended version)
 * 
 * @param name        Null-terminated UTF-16 variable name
 * @param guid        Pointer to EFI_GUID identifying the variable namespace
 * @param attributes  Output: variable attributes (see EFI_VARIABLE_* constants)
 * @param buffer      Pointer to output buffer (can be NULL to query size)
 * @param buffer_size IN: size of buffer / OUT: required size or actual data size
 * @return            EFI_STATUS result
 */
EFI_STATUS uefi_get_var_ex(
    IN CHAR16* name,
    IN EFI_GUID* guid,
    OUT UINT32* attributes,
    OUT VOID* buffer,
    IN OUT UINTN* buffer_size
);

/**
 * Delete a UEFI Variable (set data size to 0)
 * 
 * @param name        Null-terminated UTF-16 variable name
 * @param guid        Pointer to EFI_GUID identifying the variable namespace
 * @return            EFI_STATUS result
 */
EFI_STATUS uefi_delete_var(
    IN CHAR16* name,
    IN EFI_GUID* guid
);

/**
 * Enumerate all variables in a namespace
 * 
 * @param guid        Pointer to EFI_GUID (NULL for all namespaces)
 * @param callback    Function called for each variable found
 * @param context     User context passed to callback
 * @return            EFI_STATUS result
 */
typedef EFI_STATUS (*UEFI_VAR_ENUM_CALLBACK)(
    IN CHAR16* name,
    IN EFI_GUID* guid,
    IN UINT32 attributes,
    IN VOID* data,
    IN UINTN data_size,
    IN VOID* context
);

EFI_STATUS uefi_enum_vars(
    IN EFI_GUID* guid,
    IN UEFI_VAR_ENUM_CALLBACK callback,
    IN VOID* context
);

#endif /* UEFI_NVRAM_H */