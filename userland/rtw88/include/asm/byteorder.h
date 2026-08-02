#ifndef _ASM_BYTEORDER_H
#define _ASM_BYTEORDER_H

#define __LITTLE_ENDIAN 1234
#define __BIG_ENDIAN 4321
#define __BYTEORDER_HAS_U64__

#include <linux/types.h>

#ifndef __cpu_to_le16
#define __cpu_to_le16(x) ((__le16)(__u16)(x))
#endif
#ifndef __cpu_to_le32
#define __cpu_to_le32(x) ((__le32)(__u32)(x))
#endif
#ifndef __cpu_to_le64
#define __cpu_to_le64(x) ((__le64)(__u64)(x))
#endif
#ifndef __le16_to_cpu
#define __le16_to_cpu(x) ((__u16)(x))
#endif
#ifndef __le32_to_cpu
#define __le32_to_cpu(x) ((__u32)(x))
#endif
#ifndef __le64_to_cpu
#define __le64_to_cpu(x) ((__u64)(x))
#endif

#ifndef cpu_to_le16
#define cpu_to_le16(x) __cpu_to_le16(x)
#endif
#ifndef cpu_to_le32
#define cpu_to_le32(x) __cpu_to_le32(x)
#endif
#ifndef cpu_to_le64
#define cpu_to_le64(x) __cpu_to_le64(x)
#endif
#ifndef le16_to_cpu
#define le16_to_cpu(x) __le16_to_cpu(x)
#endif
#ifndef le32_to_cpu
#define le32_to_cpu(x) __le32_to_cpu(x)
#endif
#ifndef le64_to_cpu
#define le64_to_cpu(x) __le64_to_cpu(x)
#endif

#ifndef cpu_to_be16
#define cpu_to_be16(x) ((__be16)(x))
#endif
#ifndef cpu_to_be32
#define cpu_to_be32(x) ((__be32)(x))
#endif
#ifndef be16_to_cpu
#define be16_to_cpu(x) ((__u16)(x))
#endif
#ifndef be32_to_cpu
#define be32_to_cpu(x) ((__u32)(x))
#endif

#endif
