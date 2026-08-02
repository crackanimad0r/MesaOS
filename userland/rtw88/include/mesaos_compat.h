#ifndef __MESAOS_COMPAT_H
#define __MESAOS_COMPAT_H

typedef unsigned long size_t;
typedef long ssize_t;
typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long long u64;
typedef signed char s8;
typedef short s16;
typedef int s32;
typedef long long s64;
typedef u16 __le16;
typedef u32 __le32;
typedef u64 __le64;
typedef u16 __be16;
typedef u32 __be32;
typedef u64 __be64;
typedef u32 gfp_t;
typedef u64 dma_addr_t;
typedef u64 phys_addr_t;
typedef u32 netdev_features_t;
typedef int pid_t;

#define bool _Bool
#define true 1
#define false 0

#define NULL ((void*)0)

enum { false_val = 0, true_val = 1 };

#pragma GCC diagnostic ignored "-Waddress-of-packed-member"
#pragma GCC diagnostic ignored "-Wstrict-aliasing"

#define __packed __attribute__((__packed__))
#define __aligned(x) __attribute__((__aligned__(x)))
#define __always_unused __attribute__((__unused__))
#define __maybe_unused __attribute__((__unused__))
#define __must_check __attribute__((__warn_unused_result__))
#define __user
#define __force
#define __iomem
#define likely(x) __builtin_expect(!!(x), 1)
#define unlikely(x) __builtin_expect(!!(x), 0)

#define offsetof(TYPE, MEMBER) __builtin_offsetof(TYPE, MEMBER)
#define container_of(ptr, type, member) ({ \
    const typeof(((type *)0)->member) *__mptr = (ptr); \
    (type *)((char *)__mptr - offsetof(type, member)); })

#define ARRAY_SIZE(arr) (sizeof(arr) / sizeof((arr)[0]))
#define DIV_ROUND_UP(n,d) (((n) + (d) - 1) / (d))
#define DIV_ROUND_CLOSEST(n, d) ({ typeof(n) _n = (n); typeof(d) _d = (d); (_n + _d / 2) / _d; })
#define ALIGN(x,a) (((x) + (a) - 1) & ~((typeof(x))(a) - 1))
#define ALIGN_DOWN(x,a) ((x) & ~((typeof(x))(a) - 1))
#define min(x,y) ((x) < (y) ? (x) : (y))
#define max(x,y) ((x) > (y) ? (x) : (y))
#define clamp(v, lo, hi) min(max(v, lo), hi)
#define abs(x) ((x) < 0 ? -(x) : (x))
#define swap(a,b) do { typeof(a) __tmp = (a); (a) = (b); (b) = __tmp; } while(0)

#define BIT(nr) (1UL << (nr))
#define BIT_ULL(nr) (1ULL << (nr))
#define BIT_MASK(nr) (1UL << ((nr) % 64))
#define BIT_WORD(nr) ((nr) / 64)
#define BITS_PER_LONG 64
#define BITS_TO_LONGS(nr) DIV_ROUND_UP(nr, BITS_PER_LONG)
#define GENMASK(h,l) (((~0UL) << (l)) & (~0UL >> (64-1-(h))))
#define GENMASK_ULL(h,l) (((~0ULL) << (l)) & (~0ULL >> (64-1-(h))))
#define __set_bit(nr, addr) ((void)(*(addr) |= BIT(nr)))
#define test_bit(nr, addr) ((*(addr) >> (nr)) & 1)

#define BUILD_BUG_ON(cond) ((void)sizeof(char[1 - 2 * !!(cond)]))
#define BUILD_BUG_ON_MSG(cond, msg) BUILD_BUG_ON(cond)

#define ETH_ALEN 6
#define ETH_P_80211_MINIMAL 0x00f0

#define IEEE80211_NUM_ACS 4
#define IEEE80211_NUM_TIDS 9
#define IEEE80211_MLD_MAX_NUM_LINKS 15
#define IEEE80211_MAX_CHAINS 4

enum nl80211_band {
    NL80211_BAND_2GHZ = 0,
    NL80211_BAND_5GHZ = 1,
    NL80211_BAND_60GHZ = 2,
};

enum nl80211_iftype {
    NL80211_IFTYPE_UNSPECIFIED = 0,
    NL80211_IFTYPE_ADHOC = 1,
    NL80211_IFTYPE_STATION = 2,
    NL80211_IFTYPE_AP = 3,
    NL80211_IFTYPE_MESH_POINT = 5,
    NL80211_IFTYPE_P2P_CLIENT = 7,
    NL80211_IFTYPE_P2P_GO = 8,
};

#define IEEE80211_CONF_MONITOR (1<<0)
#define IEEE80211_CONF_PS (1<<1)
#define IEEE80211_CONF_IDLE (1<<2)
#define IEEE80211_CONF_OFFCHANNEL (1<<3)

#define IEEE80211_SMPS_AUTOMATIC 0
#define IEEE80211_SMPS_OFF 1
#define IEEE80211_SMPS_STATIC 2
#define IEEE80211_SMPS_DYNAMIC 3

#define RX_ENC_HT 1
#define RX_ENC_VHT 2

#define RATE_INFO_BW_20 0
#define RATE_INFO_BW_40 1
#define RATE_INFO_BW_80 2

#define RX_FLAG_MMIC_ERROR 0x00000001
#define RX_FLAG_DECRYPTED 0x00000002
#define RX_FLAG_MMIC_STRIPPED 0x00000010
#define RX_FLAG_IV_STRIPPED 0x00000020
#define RX_FLAG_FAILED_FCS_CRC 0x00000040
#define RX_FLAG_MACTIME_START 0x00000200
#define RX_FLAG_AMPDU_DETAILS 0x40000000

#define IEEE80211_TX_STAT_ACK 0x00000001
#define IEEE80211_TX_STAT_AMPDU 0x00000020

#define IEEE80211_HW_SIGNAL_DBM 4
#define IEEE80211_HW_RX_INCLUDES_FCS 1
#define IEEE80211_HW_AMPDU_AGGREGATION 7
#define IEEE80211_HW_MFP_CAPABLE 11
#define IEEE80211_HW_REPORTS_TX_ACK_STATUS 17
#define IEEE80211_HW_SUPPORTS_PS 8
#define IEEE80211_HW_SUPPORTS_DYNAMIC_PS 10
#define IEEE80211_HW_SUPPORT_FAST_XMIT 16
#define IEEE80211_HW_SUPPORTS_AMSDU_IN_AMPDU 31
#define IEEE80211_FCTL_TODS 0x0001
#define IEEE80211_FCTL_FROMDS 0x0002

#ifndef cpu_to_le16
#define cpu_to_le16(x) (x)
#endif
#ifndef cpu_to_le32
#define cpu_to_le32(x) (x)
#endif

#define IEEE80211_HW_HAS_RATE_CONTROL 0
#define IEEE80211_HW_SINGLE_SCAN_ON_ALL_BANDS 29
#define IEEE80211_HW_TX_AMSDU 36

#define HZ 100

#define GFP_KERNEL 0
#define GFP_ATOMIC 0

#define IRQ_HANDLED 1
#define IRQ_NONE 0
#define IRQF_SHARED 0x00000001
#define IRQF_TRIGGER_LOW 0x00000100
#define IRQF_TRIGGER_HIGH 0x00000200

#define EPERM 1
#define ENOENT 2
#define ESRCH 3
#define EINTR 4
#define EIO 5
#define ENXIO 6
#define E2BIG 7
#define ENOEXEC 8
#define EBADF 9
#define ECHILD 10
#define EAGAIN 11
#define ENOMEM 12
#define EACCES 13
#define EFAULT 14
#define ENOTBLK 15
#define EBUSY 16
#define EEXIST 17
#define EXDEV 18
#define ENODEV 19
#define ENOTDIR 20
#define EISDIR 21
#define EINVAL 22
#define ENFILE 23
#define EMFILE 24
#define ENOTTY 25
#define ETXTBSY 26
#define EFBIG 27
#define ENOSPC 28
#define ESPIPE 29
#define EROFS 30
#define EMLINK 31
#define EPIPE 32
#define EDOM 33
#define ERANGE 34
#define EDEADLK 35
#define ENAMETOOLONG 36
#define ENOLCK 37
#define ENOSYS 38
#define ENOTEMPTY 39
#define ELOOP 40
#define EWOULDBLOCK EAGAIN
#define ENOMSG 42
#define EIDRM 43
#define ECHRNG 44
#define EL2NSYNC 45
#define EL3HLT 46
#define EL3RST 47
#define ELNRNG 48
#define EUNATCH 49
#define ENOCSI 50
#define EL2HLT 51
#define EBADE 52
#define EBADR 53
#define EXFULL 54
#define ENOANO 55
#define EBADRQC 56
#define EBADSLT 57
#define EDEADLOCK EDEADLK
#define EBFONT 59
#define ENOSTR 60
#define ENODATA 61
#define ETIME 62
#define ENOSR 63
#define ENONET 64
#define ENOPKG 65
#define EREMOTE 66
#define ENOLINK 67
#define EADV 68
#define ESRMNT 69
#define ECOMM 70
#define EPROTO 71
#define EMULTIHOP 72
#define EDOTDOT 73
#define EBADMSG 74
#define EOVERFLOW 75
#define ENOTUNIQ 76
#define EBADFD 77
#define EREMCHG 78
#define ELIBACC 79
#define ELIBBAD 80
#define ELIBSCN 81
#define ELIBMAX 82
#define ELIBEXEC 83
#define EILSEQ 84
#define ERESTART 85
#define ESTRPIPE 86
#define EUSERS 87
#define ENOTSOCK 88
#define EDESTADDRREQ 89
#define EMSGSIZE 90
#define EPROTOTYPE 91
#define ENOPROTOOPT 92
#define EPROTONOSUPPORT 93
#define ESOCKTNOSUPPORT 94
#define EOPNOTSUPP 95
#define EPFNOSUPPORT 96
#define EAFNOSUPPORT 97
#define EADDRINUSE 98
#define EADDRNOTAVAIL 99
#define ENETDOWN 100
#define ENETUNREACH 101
#define ENETRESET 102
#define ECONNABORTED 103
#define ECONNRESET 104
#define ENOBUFS 105
#define EISCONN 106
#define ENOTCONN 107
#define ESHUTDOWN 108
#define ETOOMANYREFS 109
#define ETIMEDOUT 110
#define ECONNREFUSED 111
#define EHOSTDOWN 112
#define EHOSTUNREACH 113
#define EALREADY 114
#define EINPROGRESS 115
#define ESTALE 116
#define EUCLEAN 117
#define ENOTNAM 118
#define ENAVAIL 119
#define EISNAM 120
#define EREMOTEIO 121
#define EDQUOT 122
#define ENOMEDIUM 123
#define EMEDIUMTYPE 124
#define ECANCELED 125
#define ENOKEY 126
#define EKEYEXPIRED 127
#define EKEYREVOKED 128
#define EKEYREJECTED 129
#define EOWNERDEAD 130
#define ENOTRECOVERABLE 131

#define SMP_CACHE_BYTES 64
#define L1_CACHE_BYTES 64

#define NR_IRQS 256

struct list_head {
    struct list_head *next, *prev;
};

static inline void INIT_LIST_HEAD(struct list_head *list) {
    list->next = list;
    list->prev = list;
}

static inline void __list_add(struct list_head *new, struct list_head *prev, struct list_head *next) {
    next->prev = new;
    new->next = next;
    new->prev = prev;
    prev->next = new;
}

static inline void list_add(struct list_head *new, struct list_head *head) {
    __list_add(new, head, head->next);
}

static inline void list_add_tail(struct list_head *new, struct list_head *head) {
    __list_add(new, head->prev, head);
}

static inline void __list_del(struct list_head *prev, struct list_head *next) {
    next->prev = prev;
    prev->next = next;
}

static inline void list_del(struct list_head *entry) {
    __list_del(entry->prev, entry->next);
}

static inline int list_empty(const struct list_head *head) {
    return head->next == head;
}

#define list_entry(ptr, type, member) container_of(ptr, type, member)
#define list_first_entry(ptr, type, member) list_entry((ptr)->next, type, member)
#define list_for_each(pos, head) for (pos = (head)->next; pos != (head); pos = pos->next)
#define list_for_each_entry(pos, head, member) \
    for (pos = list_entry((head)->next, typeof(*pos), member); \
         &pos->member != (head); \
         pos = list_entry(pos->member.next, typeof(*pos), member))
#define list_for_each_safe(pos, n, head) \
    for (pos = (head)->next, n = pos->next; pos != (head); pos = n, n = pos->next)

typedef struct { int counter; } atomic_t;

static inline void atomic_set(atomic_t *v, int i) { v->counter = i; }
static inline int atomic_read(const atomic_t *v) { return v->counter; }
static inline void atomic_add(int i, atomic_t *v) { v->counter += i; }
static inline void atomic_sub(int i, atomic_t *v) { v->counter -= i; }
static inline void atomic_inc(atomic_t *v) { v->counter++; }
static inline void atomic_dec(atomic_t *v) { v->counter--; }

typedef struct { unsigned long lock; } spinlock_t;

static inline void spin_lock_init(spinlock_t *lock) { lock->lock = 0; }
static inline void spin_lock(spinlock_t *lock) { while (__sync_lock_test_and_set(&lock->lock, 1)) {} }
static inline void spin_unlock(spinlock_t *lock) { __sync_lock_release(&lock->lock); }
#define spin_lock_irqsave(lock, flags) do { (void)(flags); spin_lock(lock); } while(0)
#define spin_unlock_irqrestore(lock, flags) do { (void)(flags); spin_unlock(lock); } while(0)
static inline void spin_lock_bh(spinlock_t *lock) { spin_lock(lock); }
static inline void spin_unlock_bh(spinlock_t *lock) { spin_unlock(lock); }

struct mutex { int lock; };
typedef struct mutex mutex_t;

static inline void mutex_init(struct mutex *m) { m->lock = 0; }
static inline void mutex_lock(struct mutex *m) { while (__sync_lock_test_and_set(&m->lock, 1)) {} }
static inline void mutex_unlock(struct mutex *m) { __sync_lock_release(&m->lock); }

struct completion { unsigned long done; };

static inline void init_completion(struct completion *c) { c->done = 0; }
static inline void wait_for_completion(struct completion *c) { while (!c->done) {} }
static inline void complete(struct completion *c) { c->done = 1; }

struct sk_buff;
struct net_device;

struct sk_buff_head {
    struct sk_buff *next;
    struct sk_buff *prev;
    u32 qlen;
    spinlock_t lock;
};

struct sk_buff {
    u8 head[8];
    u8 data[8];
    u8 tail[8];
    u8 end[8];
    u32 len;
    u32 truesize;
    struct net_device *dev;
    u8 cb[48];
    unsigned long _resv1;
    struct sk_buff *next;
    struct sk_buff *prev;
    struct sk_buff_head *list;
    unsigned int _resv2;
    u16 priority;
    u16 protocol;
};

extern struct sk_buff *dev_alloc_skb(unsigned int len);
extern void kfree_skb(struct sk_buff *skb);
extern unsigned char *skb_put(struct sk_buff *skb, unsigned int len);
extern unsigned char *skb_push(struct sk_buff *skb, unsigned int len);
extern void skb_reserve(struct sk_buff *skb, int len);
extern void skb_copy_to_linear_data(struct sk_buff *skb, const void *src, unsigned int len);
extern void skb_copy_from_linear_data(struct sk_buff *skb, void *dst, unsigned int len);
extern __be16 eth_type_trans(struct sk_buff *skb, struct net_device *dev);
extern int netif_rx(struct sk_buff *skb);
extern int dev_queue_xmit(struct sk_buff *skb);
extern void netif_start_queue(struct net_device *dev);
extern void netif_wake_queue(struct net_device *dev);
extern void netif_stop_queue(struct net_device *dev);
extern void netif_carrier_on(struct net_device *dev);
extern void netif_carrier_off(struct net_device *dev);

struct net_device {
    char name[16];
    unsigned long state;
    struct net_device *next;
    struct net_device *prev;
    unsigned char dev_addr[ETH_ALEN];
    unsigned char broadcast[ETH_ALEN];
    unsigned int mtu;
    unsigned short type;
    unsigned short hard_header_len;
    unsigned char priv[0] __aligned(sizeof(void *));
};

static inline void *netdev_priv(const struct net_device *dev) {
    return (void *)dev + 2048;
}

extern int register_netdev(struct net_device *dev);
extern void unregister_netdev(struct net_device *dev);
extern struct net_device *alloc_etherdev(int sizeof_priv);
extern void free_netdev(struct net_device *dev);

static inline void ether_addr_copy(u8 *dst, const u8 *src) {
    dst[0] = src[0]; dst[1] = src[1]; dst[2] = src[2];
    dst[3] = src[3]; dst[4] = src[4]; dst[5] = src[5];
}

static inline int is_broadcast_ether_addr(const u8 *addr) {
    return addr[0] == 0xff && addr[1] == 0xff && addr[2] == 0xff &&
           addr[3] == 0xff && addr[4] == 0xff && addr[5] == 0xff;
}

static inline u16 get_unaligned_le16(const void *p) {
    return *(const u16 *)p;
}

static inline u32 get_unaligned_le32(const void *p) {
    return *(const u32 *)p;
}

static inline void put_unaligned_le16(u16 v, void *p) {
    *(u16 *)p = v;
}

static inline void put_unaligned_le32(u32 v, void *p) {
    *(u32 *)p = v;
}

static inline u16 get_unaligned_be16(const void *p) {
    return __builtin_bswap16(*(const u16 *)p);
}

static inline void put_unaligned_be16(u16 v, void *p) {
    *(u16 *)p = __builtin_bswap16(v);
}

static inline u32 get_unaligned_be32(const void *p) {
    return __builtin_bswap32(*(const u32 *)p);
}

struct device {
    void *driver_data;
};

/* ── cfg80211 types ── */
struct ieee80211_channel {
    u32 band;
    u16 center_freq;
    u16 freq_offset;
    u8 hw_value;
    u32 flags;
    u8 _pad[55];
};

enum nl80211_chan_width {
    NL80211_CHAN_WIDTH_20_NOHT,
    NL80211_CHAN_WIDTH_20,
    NL80211_CHAN_WIDTH_40,
    NL80211_CHAN_WIDTH_80,
    NL80211_CHAN_WIDTH_80P80,
    NL80211_CHAN_WIDTH_160,
    NL80211_CHAN_WIDTH_5,
    NL80211_CHAN_WIDTH_10,
};

struct cfg80211_chan_def {
    struct ieee80211_channel *chan;
    enum nl80211_chan_width width;
    u32 center_freq1;
    u32 center_freq2;
};

struct ieee80211_conf {
    u32 flags;
    int power_level;
    int dynamic_ps_timeout;
    u16 listen_interval;
    u8 ps_dtim_period;
    u8 long_frame_max_tx_count;
    u8 short_frame_max_tx_count;
    u8 _pad0[7];
    struct cfg80211_chan_def chandef;
    bool radar_enabled;
    u8 _pad1[3];
    u32 smps_mode;
};

struct ieee80211_rx_status {
    u64 mactime;
    u64 boottime_ns;
    u32 device_timestamp;
    u32 ampdu_reference;
    u32 flag;
    u16 freq;
    u8 enc_flags;
    u8 encoding;
    u8 bw;
    u8 _pad0;
    u8 rate_idx;
    u8 nss;
    u8 rx_flags;
    u8 band;
    u8 antenna;
    s8 signal;
    u8 chains;
    s8 chain_signal[IEEE80211_MAX_CHAINS];
    u8 zero_length_psdu_type;
    u8 link_valid;
    u8 link_id;
};

struct ieee80211_txq {
    struct ieee80211_vif *vif;
    struct ieee80211_sta *sta;
    u8 tid;
    u8 ac;
    u8 drv_priv[0] __aligned(sizeof(void *));
};

struct ieee80211_vif_cfg {
    unsigned char bssid[ETH_ALEN];
    unsigned char _pad[2];
    u16 aid;
    bool assoc;
    bool ps;
    unsigned char _pad2[2];
};

struct ieee80211_mu_group {
    u8 membership[8];
    u8 position[16];
};

struct ieee80211_bss_conf {
    unsigned char bssid[ETH_ALEN];
    u8 _pad0[2];
    bool use_short_slot;
    u8 _pad1[3];
    u32 basic_rates;
    u16 beacon_int;
    u8 dtim_period;
    bool enable_beacon;
    s32 cqm_rssi_thold;
    u32 cqm_rssi_hyst;
    struct ieee80211_mu_group mu_group;
};

struct ieee80211_vif {
    u32 type;
    struct ieee80211_vif_cfg cfg;
    struct ieee80211_bss_conf bss_conf;
    u64 link_conf[IEEE80211_MLD_MAX_NUM_LINKS];
    u16 valid_links;
    u16 active_links;
    u16 dormant_links;
    u16 suspended_links;
    u8 addr[ETH_ALEN] __aligned(2);
    bool addr_valid;
    bool p2p;
    u8 cab_queue;
    u8 hw_queue[IEEE80211_NUM_ACS];
    struct ieee80211_txq *txq;
    u32 netdev_features;
    u32 driver_flags;
    u32 offload_flags;
    u8 drv_priv[0] __aligned(sizeof(void *));
};

struct ieee80211_sta_rates {
    u8 _data[128];
};

struct ieee80211_sta_aggregates {
    u8 _data[64];
};

struct ieee80211_sta_vht_cap {
    bool vht_supported;
    u8 _pad0[3];
    u32 cap;
    struct {
        u16 rx_mcs_map;
        u16 rx_mcs_8_15;
        u16 tx_mcs_map;
        u16 tx_mcs_8_15;
        u16 rx_highest;
        u16 tx_highest;
    } vht_mcs;
};

struct ieee80211_link_sta {
    u8 addr[ETH_ALEN];
    u8 _pad0[2];
    u16 aid;
    struct {
        bool ht_supported;
        bool vht_supported;
        u8 _pad;
        u32 cap;
        struct {
            u8 rx_mask[10];
            u8 _pad2[22];
            u16 rx_highest;
            u8 tx_params;
        } mcs;
        u8 ampdu_factor;
        u8 ampdu_density;
    } ht_cap;
    struct ieee80211_sta_vht_cap vht_cap;
    u32 supp_rates[4];
    u32 bandwidth;
    struct ieee80211_sta *sta;
    u8 _pad1[0];
    struct {
        u16 max_rc_amsdu_len;
    } agg;
};

struct ieee80211_sta {
    u8 addr[ETH_ALEN] __aligned(2);
    u16 aid;
    u16 max_rx_aggregation_subframes;
    bool wme;
    u8 uapsd_queues;
    u8 max_sp;
    struct ieee80211_sta_rates *rates;
    bool tdls;
    bool tdls_initiator;
    bool mfp;
    bool mlo;
    bool spp_amsdu;
    u8 max_amsdu_subframes;
    u16 eml_cap;
    struct ieee80211_sta_aggregates *cur;
    bool support_p2p_ps;
    struct ieee80211_txq *txq[IEEE80211_NUM_TIDS + 1];
    u16 valid_links;
    bool epp_peer;
    struct ieee80211_link_sta deflink;
    u64 link[IEEE80211_MLD_MAX_NUM_LINKS];
    u8 drv_priv[0] __aligned(sizeof(void *));
};

struct ieee80211_hdr {
    __le16 frame_control;
    __le16 duration_id;
    u8 addr1[ETH_ALEN];
    u8 addr2[ETH_ALEN];
    u8 addr3[ETH_ALEN];
    __le16 seq_ctrl;
    u8 addr4[ETH_ALEN];
} __packed __aligned(2);

struct ieee80211_hw {
    struct ieee80211_conf conf;
    struct wiphy *wiphy;
    const char *rate_control_algorithm;
    void *priv;
    unsigned long flags[BITS_TO_LONGS(57)];
    unsigned int extra_tx_headroom;
    unsigned int extra_beacon_tailroom;
    int vif_data_size;
    int sta_data_size;
    int chanctx_data_size;
    int txq_data_size;
    u16 queues;
    u16 max_listen_interval;
    s8 max_signal;
    u8 max_rates;
    u8 max_report_rates;
    u8 max_rate_tries;
    u16 max_rx_aggregation_subframes;
    u16 max_tx_aggregation_subframes;
    u8 max_tx_fragments;
    u8 offchannel_tx_hw_queue;
    u8 radiotap_mcs_details;
    u8 _pad0;
    u16 radiotap_vht_details;
    struct { int units_pos; s16 accuracy; } radiotap_timestamp;
    netdev_features_t netdev_features;
    u8 uapsd_queues;
    u8 uapsd_max_sp_len;
    u8 max_nan_de_entries;
    u8 tx_sk_pacing_shift;
    u8 weight_multiplier;
    u8 _pad1[3];
    u32 max_mtu;
    u8 _pad2[4];
    const s8 *tx_power_levels;
    u8 max_txpwr_levels_idx;
    u8 _pad3[15];
};

/* ── cfg80211 / ieee80211 ops structs ── */

struct ieee80211_scan_request;

struct ieee80211_ops {
    void *tx;
    void *start;
    void *stop;
    void *add_interface;
    void *remove_interface;
    void *change_interface;
    void *config;
    void *configure_filter;
    void *bss_info_changed;
    void *sta_add;
    void *sta_remove;
    void *sta_state;
    void *sta_rc_update;
    void *link_sta_rc_update;
    void *set_key;
    void *conf_tx;
    void *sw_scan_start;
    void *sw_scan_complete;
    void *get_survey;
    void *get_et_sset_count;
    void *get_et_strings;
    void *get_et_stats;
    void *set_rts_threshold;
    void *set_coverage_class;
    void *set_antenna;
    void *get_antenna;
    void *set_bitrate_mask;
    void *set_wiphy_params;
    void *set_tx_power;
    void *get_tx_power;
    void *set_cqm_rssi_config;
    void *sched_scan_start;
    void *sched_scan_stop;
    void *set_power_mgmt;
    void *set_rekey_data;
    void *change_beacon;
    void *flush;
    void *remain_on_channel;
    void *cancel_remain_on_channel;
    void *add_chanctx;
    void *remove_chanctx;
    void *change_chanctx;
    void *switch_vif_chanctx;
    void *wake_tx_queue;
    void *start_ap;
    void *stop_ap;
    void *set_tim;
    void *ampdu_action;
    void *can_aggregate_in_amsdu;
    void *mgd_prepare_tx;
    void *sta_statistics;
    void *reconfig_complete;
    void *hw_scan;
    void *cancel_hw_scan;
    void *set_sar_specs;
};

struct wireless_dev {
    struct wiphy *wiphy;
};

struct firmware {
    size_t size;
    const u8 *data;
};

struct napi_struct {
    u8 _data[64];
};

/* ── EWMA ── */
#define DECLARE_EWMA(name, p, w) \
    struct ewma_##name { unsigned long internal; }; \
    static inline void ewma_##name##_init(struct ewma_##name *e) { e->internal = 0; } \
    static inline unsigned long ewma_##name##_read(struct ewma_##name *e) { return e->internal; } \
    static inline void ewma_##name##_add(struct ewma_##name *e, unsigned long val) { e->internal = val; }

/* ── pci types ── */

struct pci_dev;

struct pci_device_id {
    u32 vendor, device;
    u32 subvendor, subdevice;
    u32 class, class_mask;
    unsigned long driver_data;
};

struct device_driver {
    const struct dev_pm_ops *pm;
};

struct pci_driver {
    const char *name;
    const struct pci_device_id *id_table;
    int (*probe)(struct pci_dev *dev, const struct pci_device_id *id);
    void (*remove)(struct pci_dev *dev);
    struct device_driver driver;
    void (*shutdown)(struct pci_dev *dev);
    const struct pci_error_handlers *err_handler;
};

struct device;

struct pci_dev {
    struct device dev;
    unsigned int vendor;
    unsigned int device;
    unsigned int subsystem_vendor;
    unsigned int subsystem_device;
    unsigned int devfn;
    unsigned short class;
    u8 revision;
    unsigned int irq;
};

/* ── Forward declarations ── */

struct work_struct;
struct delayed_work;
struct timer_list;
struct napi_struct;
struct ieee80211_ops;
struct ieee80211_sta;
struct ieee80211_vif;
struct ieee80211_hw;
struct ieee80211_txq;
struct ieee80211_hdr;
struct sk_buff;
struct net_device;
struct firmware;
struct seq_file;
struct device;
struct module;
struct cfg80211_scan_info;

/* ── mac80211 function declarations ── */

extern struct ieee80211_hw *ieee80211_alloc_hw(int sizeof_priv, const struct ieee80211_ops *ops);
extern int ieee80211_register_hw(struct ieee80211_hw *hw);
extern void ieee80211_unregister_hw(struct ieee80211_hw *hw);
extern void ieee80211_free_hw(struct ieee80211_hw *hw);
extern void ieee80211_stop_queues(struct ieee80211_hw *hw);
extern void ieee80211_wake_queues(struct ieee80211_hw *hw);
extern void ieee80211_stop_queue(struct ieee80211_hw *hw, u32 queue);
extern void ieee80211_wake_queue(struct ieee80211_hw *hw, u32 queue);
extern void ieee80211_tx_status_irqsafe(struct ieee80211_hw *hw, struct sk_buff *skb);
extern void ieee80211_rx_napi(struct ieee80211_hw *hw, struct ieee80211_sta *sta, struct sk_buff *skb, struct napi_struct *napi);
extern void ieee80211_rx_irqsafe(struct ieee80211_hw *hw, struct sk_buff *skb);
extern struct ieee80211_sta *ieee80211_find_sta(struct ieee80211_vif *vif, const u8 *addr);
extern struct ieee80211_sta *ieee80211_find_sta_by_ifaddr(struct ieee80211_hw *hw, const u8 *addr1, const u8 *addr2);
extern void ieee80211_iterate_stations_atomic(struct ieee80211_hw *hw, void (*iterator)(void *data, struct ieee80211_sta *sta), void *data);
extern struct sk_buff *ieee80211_beacon_get_tim(struct ieee80211_hw *hw, struct ieee80211_vif *vif, u16 *tim_offset, u32 *tim_length, u32 *reserved);
extern void ieee80211_scan_completed(struct ieee80211_hw *hw, const struct cfg80211_scan_info *info);
extern void ieee80211_connection_loss(struct ieee80211_vif *vif);
extern void ieee80211_queue_work(struct ieee80211_hw *hw, struct work_struct *work);
extern void ieee80211_queue_delayed_work(struct ieee80211_hw *hw, struct delayed_work *dwork, unsigned long delay);
extern int ieee80211_channel_to_frequency(int chan, u32 band);
extern void ieee80211_free_txskb(struct ieee80211_hw *hw, struct sk_buff *skb);
extern struct sk_buff *ieee80211_tx_dequeue(struct ieee80211_hw *hw, struct ieee80211_txq *txq);
extern void ieee80211_tx_info_clear_status(void *status);
extern void ieee80211_txq_get_depth(struct ieee80211_txq *txq, unsigned long *frame_cnt, unsigned long *byte_cnt);
extern int ieee80211_start_tx_ba_session(struct ieee80211_sta *sta, u16 tid, u16 timeout);
extern void ieee80211_stop_tx_ba_cb_irqsafe(struct ieee80211_vif *vif, const u8 *addr, u16 tid);
extern void ieee80211_purge_tx_queue(struct ieee80211_hw *hw, struct sk_buff_head *queues);
extern void ieee80211_restart_hw(struct ieee80211_hw *hw);
extern int ieee80211_request_smps(struct ieee80211_vif *vif, unsigned int link_id, u32 smps_mode);
extern void ieee80211_cqm_rssi_notify(struct ieee80211_vif *vif, u32 event, s32 sig, gfp_t gfp);
extern void ieee80211_report_wowlan_wakeup(struct ieee80211_vif *vif, void *wakeup, gfp_t gfp);
extern void ieee80211_iterate_active_interfaces_atomic(struct ieee80211_hw *hw,
    unsigned int iter_flags,
    void (*iterator)(void *data, u8 *mac, struct ieee80211_vif *vif),
    void *data);
extern void *ieee80211_create_tpt_led_trigger(struct ieee80211_hw *hw, unsigned int flags, const void *blink_set, unsigned int blinks);
extern struct sk_buff *ieee80211_pspoll_get(struct ieee80211_hw *hw, struct ieee80211_vif *vif);
extern struct sk_buff *ieee80211_nullfunc_get(struct ieee80211_hw *hw, struct ieee80211_vif *vif, int link_id, bool qos);
extern struct sk_buff *ieee80211_proberesp_get(struct ieee80211_hw *hw, struct ieee80211_vif *vif);
extern struct sk_buff *ieee80211_probereq_get(struct ieee80211_hw *hw, const u8 *addr, const u8 *ssid, size_t ssid_len, size_t tailroom);

extern struct ieee80211_hw *wiphy_to_ieee80211_hw(struct wiphy *wiphy);

extern u32 cfg80211_calculate_bitrate(const void *rate);
extern bool cfg80211_ssid_eq(const void *a, const void *b);
extern int cfg80211_get_ies_channel_number(const u8 *ie, size_t ielen, u32 band);

extern void netif_napi_add(struct net_device *dev, struct napi_struct *napi, int (*poll)(struct napi_struct *, int));
extern void netif_napi_del(struct napi_struct *napi);
extern void napi_enable(struct napi_struct *napi);
extern void napi_disable(struct napi_struct *napi);

/* ── mac80211 inline helpers ── */

static inline int ieee80211_hw_check(struct ieee80211_hw *hw, u32 flg) {
    return test_bit(flg, hw->flags);
}

static inline void ieee80211_hw_set(struct ieee80211_hw *hw, u32 flg) {
    __set_bit(flg, hw->flags);
}

static inline u32 ieee80211_vif_type_p2p(struct ieee80211_vif *vif) {
    return vif->type;
}

static inline bool ieee80211_has_tods(__le16 fc) { return fc & cpu_to_le16(IEEE80211_FCTL_TODS); }
static inline bool ieee80211_has_fromds(__le16 fc) { return fc & cpu_to_le16(IEEE80211_FCTL_FROMDS); }
static inline u32 ieee80211_is_data(__le16 fc) { return 0; }
static inline u32 ieee80211_is_mgmt(__le16 fc) { return 0; }
static inline u32 ieee80211_is_ctl(__le16 fc) { return 0; }
static inline u32 ieee80211_is_nullfunc(__le16 fc) { return 0; }
static inline u32 ieee80211_is_beacon(__le16 fc) { return 0; }
static inline u32 ieee80211_is_probe_resp(__le16 fc) { return 0; }

static inline void *ieee80211_get_band(struct wiphy *wiphy, int band) { return NULL; }

static inline void ieee80211_get_tx_rates(struct ieee80211_vif *vif, struct ieee80211_sta *sta, struct sk_buff *skb, void *dest, int max_rates) {}

/* ── sk_buff / net_device inline helpers ── */

static inline int skb_queue_len(const struct sk_buff_head *list) { return list->qlen; }
static inline void skb_queue_head_init(struct sk_buff_head *list) {
    spin_lock_init(&list->lock);
    list->prev = (struct sk_buff *)list;
    list->next = (struct sk_buff *)list;
    list->qlen = 0;
}

extern void __skb_queue_tail(struct sk_buff_head *list, struct sk_buff *newsk);
extern void skb_queue_tail(struct sk_buff_head *list, struct sk_buff *newsk);
extern struct sk_buff *skb_dequeue(struct sk_buff_head *list);
extern struct sk_buff *__skb_dequeue(struct sk_buff_head *list);
extern void skb_queue_purge(struct sk_buff_head *list);

static inline int skb_get_queue_mapping(const struct sk_buff *skb) { return 0; }
static inline void skb_set_queue_mapping(struct sk_buff *skb, u16 q) {}

static inline void *skb_get(const struct sk_buff *skb) { return (void *)skb; }

/* ── PCI inline helpers ── */

static inline void *pci_get_drvdata(const struct pci_dev *pdev) { return ((struct pci_dev *)pdev)->dev.driver_data; }
static inline void pci_set_drvdata(struct pci_dev *pdev, void *data) { pdev->dev.driver_data = data; }
static inline const char *pci_name(const struct pci_dev *pdev) { return "pci"; }

/* ── cfg80211 helpers ── */

static inline void cfg80211_put_bss(void *wiphy, void *bss) {}
static inline void cfg80211_unlink_bss(void *wiphy, void *bss) {}
static inline void *cfg80211_inform_bss(void *wiphy, void *chandef, void *mgmt, size_t len, void *elems, u32 freq, u16 capability, u16 beacon_interval, u32 signal) { return NULL; }
static inline void *cfg80211_inform_bss_frame_data(void *wiphy, void *chandef, void *mgmt, size_t len, void *elems, u32 signal) { return NULL; }
static inline void *ieee80211_bss_get_elem(const void *bss, u8 id) { return NULL; }

/* ── ieee80211 frame accessors ── */

static inline u8 *ieee80211_get_SA(const u8 *hdr) { return (u8 *)hdr + 10; }
static inline u8 *ieee80211_get_DA(const u8 *hdr) { return (u8 *)hdr + 4; }
static inline u32 ieee80211_is_QoS(__le16 fc) { return 0; }
static inline u8 *ieee80211_get_qos_ctl(const u8 *hdr) { return (u8 *)hdr + 24; }

/* ── Timer / Workqueue ── */

struct timer_list {
    unsigned long expires;
    void (*function)(struct timer_list *);
    unsigned long data;
};

extern void init_timer(struct timer_list *timer);
extern void setup_timer(struct timer_list *timer, void (*func)(struct timer_list *), unsigned long data);
extern int mod_timer(struct timer_list *timer, unsigned long expires);
extern int del_timer(struct timer_list *timer);
extern int timer_pending(const struct timer_list *timer);
static inline void add_timer(struct timer_list *timer) { mod_timer(timer, timer->expires); }

typedef void (*work_func_t)(struct work_struct *work);

struct work_struct {
    unsigned long data;
    work_func_t func;
};

struct delayed_work {
    struct work_struct work;
    struct timer_list timer;
};

extern void INIT_WORK(struct work_struct *work, work_func_t func);
extern int schedule_work(struct work_struct *work);
extern int schedule_work_on(int cpu, struct work_struct *work);
extern bool flush_work(struct work_struct *work);
extern int schedule_delayed_work(struct delayed_work *dwork, unsigned long delay);

extern unsigned long volatile jiffies;
static inline unsigned long round_jiffies_relative(unsigned long j) { return j; }

/* ── Memory barriers / IO ── */

static inline void wmb(void) { __sync_synchronize(); }
static inline void rmb(void) { __sync_synchronize(); }
static inline void mb(void) { __sync_synchronize(); }
static inline void barrier(void) { asm volatile("" ::: "memory"); }

static inline u32 readl(const volatile void *addr) { return *(volatile u32 *)addr; }
static inline u16 readw(const volatile void *addr) { return *(volatile u16 *)addr; }
static inline u8 readb(const volatile void *addr) { return *(volatile u8 *)addr; }
static inline void writel(u32 v, volatile void *addr) { *(volatile u32 *)addr = v; }
static inline void writew(u16 v, volatile void *addr) { *(volatile u16 *)addr = v; }
static inline void writeb(u8 v, volatile void *addr) { *(volatile u8 *)addr = v; }

extern u8 ioread8(const void *addr);
extern u32 ioread32(const void *addr);
extern u64 ioread64(const void *addr);
extern void iowrite8(u8 val, void *addr);
extern void iowrite32(u32 val, void *addr);
extern void iowrite64(u64 val, void *addr);

static inline u32 ioread32be(const void *addr) { return __builtin_bswap32(ioread32(addr)); }

/* ── IO polling ── */

#define read_poll_timeout(op, val, cond, sleep_us, timeout_us, sleep_before_read, args...) \
    ({ unsigned int __i; \
       for (__i = 0; __i < (timeout_us) / 10 + 1; __i++) { \
           val = op(args); \
           if (cond) break; \
           udelay(10); \
       } \
       (cond) ? 0 : -ETIMEDOUT; })

#define readl_poll_timeout(addr, val, cond, delay_us, timeout_us) \
    read_poll_timeout(readl, val, cond, delay_us, timeout_us, false, addr)

#define read_poll_timeout_atomic(op, val, cond, delay_us, timeout_us, sleep_before_read, args...) \
    ({ unsigned int __i; \
       if (sleep_before_read) udelay(delay_us); \
       for (__i = 0; __i < (timeout_us) / (delay_us ? delay_us : 1) + 1; __i++) { \
           val = op(args); \
           if (cond) break; \
           if (delay_us) udelay(delay_us); \
       } \
       (cond) ? 0 : -ETIMEDOUT; })

#define readl_poll_timeout_atomic(addr, val, cond, delay_us, timeout_us) \
    read_poll_timeout_atomic(readl, val, cond, delay_us, timeout_us, false, addr)

/* ── Bitfield macros ── */

#define FIELD_PREP(_mask, _val) \
    (((typeof(_mask))(_val) << (__builtin_ffsll(_mask) - 1)) & (_mask))
#define FIELD_GET(_mask, _val) \
    ((typeof(_mask))((_val) & (_mask)) >> (__builtin_ffsll(_mask) - 1))
#define u32_encode_bits(val, mask) FIELD_PREP(mask, val)
#define u16_encode_bits(val, mask) FIELD_PREP(mask, val)

/* ── printk / dev_* macros ── */

#define pr_info(fmt, ...) printk(fmt, ##__VA_ARGS__)
#define pr_err(fmt, ...) printk(fmt, ##__VA_ARGS__)
#define pr_warn(fmt, ...) printk(fmt, ##__VA_ARGS__)
#define pr_debug(fmt, ...) printk(fmt, ##__VA_ARGS__)
#define pr_emerg(fmt, ...) printk(fmt, ##__VA_ARGS__)

#define dev_info(dev, fmt, ...) printk(fmt, ##__VA_ARGS__)
#define dev_err(dev, fmt, ...) printk(fmt, ##__VA_ARGS__)
#define dev_warn(dev, fmt, ...) printk(fmt, ##__VA_ARGS__)
#define dev_dbg(dev, fmt, ...) printk(fmt, ##__VA_ARGS__)

/* ── rtnl / locking ── */

static inline int rtnl_lock(void) { return 0; }
static inline void rtnl_unlock(void) {}
static inline void might_sleep(void) {}
static inline void wiphy_lock(struct wiphy *w) {}
static inline void wiphy_unlock(struct wiphy *w) {}
static inline void wiphy_rfkill_set_hw_state(struct wiphy *w, bool v) {}
static inline void preempt_disable(void) {}
static inline void preempt_enable(void) {}
static inline void local_bh_disable(void) {}
static inline void local_bh_enable(void) {}
struct task_struct;
static inline void schedule(void) {}

/* ── Module / section attributes ── */

#define MODULE_LICENSE(x)
#define MODULE_AUTHOR(x)
#define MODULE_DESCRIPTION(x)
#define MODULE_VERSION(x)
#define MODULE_FIRMWARE(x)
#define MODULE_DEVICE_TABLE(x, y)
#define EXPORT_SYMBOL(x)
#define EXPORT_SYMBOL_GPL(x)
#define EXPORT_SYMBOL_NS(x, ns)
#define THIS_MODULE 0

#define __init __attribute__((__section__(".init.text")))
#define __exit __attribute__((__section__(".exit.text")))

#define module_init(x) extern int init_module(void) __attribute__((alias(#x)));
#define module_exit(x) extern void cleanup_module(void) __attribute__((alias(#x)));

#define module_param(name, type, perm)
#define module_param_named(name, value, type, perm)
#define MODULE_PARM_DESC(param, desc)

/* ── dev_coredump ── */

static inline void dev_coredumpv(struct device *dev, const void *data, size_t size, gfp_t gfp) {}

/* ── Additional IEEE802.11 types and macros ── */

#define IEEE80211_MAX_SSID_LEN 32
#define IEEE80211_SCTL_SEQ 0xFFF0

enum ieee80211_ac_numbers {
    IEEE80211_AC_VO = 0,
    IEEE80211_AC_VI = 1,
    IEEE80211_AC_BE = 2,
    IEEE80211_AC_BK = 3,
};

#define WLAN_CIPHER_SUITE_WEP40  0x000FAC01
#define WLAN_CIPHER_SUITE_WEP104 0x000FAC05
#define WLAN_CIPHER_SUITE_TKIP   0x000FAC02
#define WLAN_CIPHER_SUITE_CCMP   0x000FAC04

#define IEEE80211_VHT_MCS_SUPPORT_0_7 0
#define IEEE80211_VHT_MCS_SUPPORT_0_8 1
#define IEEE80211_VHT_MCS_SUPPORT_0_9 2

#define RATE_INFO_FLAGS_VHT_MCS   BIT(0)
#define RATE_INFO_FLAGS_MCS       BIT(1)
#define RATE_INFO_FLAGS_SHORT_GI  BIT(2)

#define IEEE80211_TX_CTL_AMPDU          BIT(0)
#define IEEE80211_TX_CTL_REQ_TX_STATUS  BIT(1)

#define RX_FLAG_NO_SIGNAL_VAL BIT(0)
#define RX_FLAG_NO_PSDU       BIT(1)

#define NL80211_CQM_RSSI_THRESHOLD_EVENT_LOW  0
#define NL80211_CQM_RSSI_THRESHOLD_EVENT_HIGH 1

struct ieee80211_tx_control {
    struct ieee80211_sta *sta;
};

struct ieee80211_tx_info {
    struct {
        struct ieee80211_vif *vif;
        struct ieee80211_key_conf *hw_key;
        struct ieee80211_sta *sta;
        u8 use_rts;
    } control;
    struct {
        u8 ampdu_ack_len;
        u8 ampdu_len;
        u8 ack_signal;
        unsigned long status_driver_data[4];
    } status;
    u32 flags;
    u32 _pad[8];
};

#define IEEE80211_SKB_CB(skb) ((struct ieee80211_tx_info *)((skb)->cb))

#define timer_container_of(var, callback_timer, timer_fieldname) \
    container_of(callback_timer, typeof(*var), timer_fieldname)

struct ieee80211_mgmt {
    __le16 frame_control;
    __le16 duration;
    u8 da[ETH_ALEN];
    u8 sa[ETH_ALEN];
    u8 bssid[ETH_ALEN];
    __le16 seq_ctrl;
    union {
        struct {
            __le64 timestamp;
            __le16 beacon_interval;
            __le16 capab_info;
            u8 variable[];
        } beacon;
        struct {
            __le64 timestamp;
            __le16 beacon_interval;
            __le16 capab_info;
            u8 variable[];
        } probe_resp;
    } u;
} __packed __aligned(2);

struct ieee80211_hdr_3addr {
    __le16 frame_control;
    __le16 duration_id;
    u8 addr1[ETH_ALEN];
    u8 addr2[ETH_ALEN];
    u8 addr3[ETH_ALEN];
    __le16 seq_ctrl;
} __packed __aligned(2);

struct cfg80211_wowlan { u8 _data[64]; };

struct cfg80211_match_set {
    struct { u8 ssid[IEEE80211_MAX_SSID_LEN]; u8 ssid_len; } ssid;
};

/* ── Endian helpers ── */

#ifndef __le16_to_cpu
#define __le16_to_cpu(x) (x)
#endif
#ifndef le16_to_cpu
#define le16_to_cpu(x) (x)
#endif
#ifndef le32_to_cpu
#define le32_to_cpu(x) (x)
#endif
#ifndef cpu_to_be16
#define cpu_to_be16(x) __builtin_bswap16(x)
#endif

static inline u32 le32_to_cpup(const __le32 *p) { return *p; }

/* ── Bitfield ops on __le32 pointers ── */

static inline void le32p_replace_bits(__le32 *addr, u32 val, u32 mask)
{
    u32 old = *addr;
    *addr = (old & ~mask) | (val << (__builtin_ffs(mask) - 1));
}

static inline __le32 le32_encode_bits(u32 val, u32 mask)
{
    return (__le32)(val << (__builtin_ffs(mask) - 1));
}

static inline u32 le32_get_bits(__le32 val, u32 mask)
{
    return (val >> (__builtin_ffs(mask) - 1)) & (mask >> (__builtin_ffs(mask) - 1));
}

static inline u32 u8_get_bits(u8 val, u32 mask)
{
    return (val >> (__builtin_ffs(mask) - 1)) & (mask >> (__builtin_ffs(mask) - 1));
}

/* ── List helpers ── */

#define list_for_each_entry_safe(pos, n, head, member)              \
    for (pos = list_entry((head)->next, typeof(*pos), member),      \
         n = list_entry(pos->member.next, typeof(*pos), member);    \
         &pos->member != (head);                                    \
         pos = n, n = list_entry(n->member.next, typeof(*pos), member))

static inline void list_del_init(struct list_head *entry)
{
    __list_del(entry->prev, entry->next);
    INIT_LIST_HEAD(entry);
}

#define list_first_entry_or_null(ptr, type, member) \
    (!list_empty(ptr) ? list_first_entry(ptr, type, member) : NULL)

/* ── rate_info, wiphy ── */

struct regulatory_request;

struct rate_info {
    u16 legacy;
    u8 mcs;
    u8 nss;
    u8 bw;
    u8 flags;
    u8 _pad[58];
};

struct ieee80211_sta_ht_cap {
    bool ht_supported;
    u8 _pad[3];
    u32 cap;
    struct {
        u8 rx_mask[10];
        u8 _pad2[22];
        u16 rx_highest;
        u8 tx_params;
    } mcs;
    u8 ampdu_factor;
    u8 ampdu_density;
};

struct ieee80211_supported_band {
    struct ieee80211_channel *channels;
    struct ieee80211_rate *bitrates;
    enum nl80211_band band;
    int n_channels;
    int n_bitrates;
    struct ieee80211_sta_ht_cap ht_cap;
    struct ieee80211_sta_vht_cap vht_cap;
};

struct wiphy {
    u8 _data[256];
    struct ieee80211_supported_band *bands[3];
    u32 rts_threshold;
    u32 regulatory_flags;
    u32 flags;
    u32 features;
    u16 max_scan_ssids;
    u16 max_scan_ie_len;
    u16 available_antennas_tx;
    u16 available_antennas_rx;
    u32 interface_modes;
    const struct ieee80211_iface_combination *iface_combinations;
    int n_iface_combinations;
    const struct cfg80211_sar_capa *sar_capa;
    void (*reg_notifier)(struct wiphy *wiphy, struct regulatory_request *request);
    u8 _pad[3772];
};

static inline void *wiphy_priv(struct wiphy *wiphy) { return (void *)wiphy + sizeof(struct wiphy); }

/* ── Kernel function declarations ── */

extern int printk(const char *fmt, ...);
extern void *kmalloc(size_t size, gfp_t flags);
extern void *kzalloc(size_t size, gfp_t flags);
extern void kfree(const void *ptr);
extern void *kcalloc(size_t n, size_t size, gfp_t flags);
extern void *krealloc(const void *ptr, size_t size, gfp_t flags);
extern void *vmalloc(unsigned long size);
extern void vfree(const void *addr);
extern void *memcpy(void *dest, const void *src, size_t n);
extern void *memset(void *s, int c, size_t n);
extern void *memmove(void *dest, const void *src, size_t n);
extern int memcmp(const void *s1, const void *s2, size_t n);
extern size_t strlen(const char *s);
extern int strcmp(const char *s1, const char *s2);
extern int strncmp(const char *s1, const char *s2, size_t n);
extern char *strcpy(char *dest, const char *src);
extern char *strncpy(char *dest, const char *src, size_t n);
extern char *strcat(char *dest, const char *src);
extern int snprintf(char *buf, size_t size, const char *fmt, ...);
extern int sprintf(char *buf, const char *fmt, ...);
extern unsigned long long get_cycles(void);
extern void msleep(unsigned int msecs);
extern unsigned int mdelay(unsigned int msecs);
extern void udelay(unsigned int usecs);
extern void ssleep(unsigned int seconds);

extern u8 inb(u16 port);
extern u16 inw(u16 port);
extern u32 inl(u16 port);
extern void outb(u16 port, u8 val);
extern void outw(u16 port, u16 val);
extern void outl(u16 port, u32 val);

extern void *ioremap(u64 phys_addr, u64 size);
extern void iounmap(void *addr);

extern int request_firmware(const struct firmware **fw, const char *name, struct device *dev);
extern void release_firmware(const struct firmware *fw);

extern void __iomem *ioport_map(unsigned long port, unsigned int nr);
extern void ioport_unmap(void __iomem *addr);

extern unsigned long copy_to_user(void *to, const void *from, unsigned long n);
extern unsigned long copy_from_user(void *to, const void *from, unsigned long n);

extern int pci_read_config_byte(const struct pci_dev *dev, int where, u8 *val);
extern int pci_read_config_word(const struct pci_dev *dev, int where, u16 *val);
extern int pci_read_config_dword(const struct pci_dev *dev, int where, u32 *val);
extern int pci_write_config_byte(const struct pci_dev *dev, int where, u8 val);
extern int pci_write_config_word(const struct pci_dev *dev, int where, u16 val);
extern int pci_write_config_dword(const struct pci_dev *dev, int where, u32 val);
extern int pci_enable_device(struct pci_dev *dev);
extern void pci_disable_device(struct pci_dev *dev);
extern void pci_set_master(struct pci_dev *dev);
extern int pci_request_regions(struct pci_dev *dev, const char *name);
extern void pci_release_regions(struct pci_dev *dev);
extern unsigned long pci_resource_start(const struct pci_dev *dev, int bar);
extern unsigned long pci_resource_end(const struct pci_dev *dev, int bar);
extern unsigned long pci_resource_len(const struct pci_dev *dev, int bar);
extern int pci_register_driver(struct pci_driver *drv);
extern void pci_unregister_driver(struct pci_driver *drv);

#define KBUILD_MODNAME "rtw88"

typedef unsigned long kernel_ulong_t;

#define PCI_DEVICE(vend, dev) .vendor = (vend), .device = (dev), \
    .subvendor = 0xffff, .subdevice = 0xffff

#define PCI_VENDOR_ID_REALTEK 0x10ec

struct dev_pm_ops { int _data; };

struct pci_error_handlers {
    int (*error_detected)(struct pci_dev *dev, int state);
    int (*mmio_enabled)(struct pci_dev *dev);
    int (*slot_reset)(struct pci_dev *dev);
    void (*reset_prepare)(struct pci_dev *dev);
    void (*reset_done)(struct pci_dev *dev);
    void (*resume)(struct pci_dev *dev);
};

static inline int __pci_register_driver(struct pci_driver *drv, struct module *mod, const char *name)
{
    return pci_register_driver(drv);
}
#undef module_init
#undef module_exit
#define module_init(x) int __init init_module(void) { return x(); }
#define module_exit(x) void __exit cleanup_module(void) { x(); }

#define module_pci_driver(drv) \
    static int __init rtw_init(void) { return __pci_register_driver(&(drv), THIS_MODULE, KBUILD_MODNAME); } \
    static void __exit rtw_exit(void) { pci_unregister_driver(&(drv)); } \
    module_init(rtw_init); \
    module_exit(rtw_exit)

extern int request_irq(unsigned int irq, void *handler, unsigned long flags, const char *name, void *dev);
extern void free_irq(unsigned int irq, void *dev_id);
extern void enable_irq(unsigned int irq);
extern void disable_irq(unsigned int irq);
extern void synchronize_irq(unsigned int irq);

extern void tasklet_init(void *t, void (*func)(unsigned long), unsigned long data);
extern void tasklet_schedule(void *t);
extern void tasklet_kill(void *t);
extern void tasklet_hi_schedule(void *t);

extern void *memdup_user(const void *src, size_t len);
extern void *memdup_user_nul(const void *src, size_t len);

#define WARN(cond, fmt, ...) ({ if (cond) printk(fmt, ##__VA_ARGS__); 0; })


/* ── Missing kernel helpers ── */

static inline void *kmemdup(const void *src, size_t len, gfp_t gfp)
{
    void *p = kmalloc(len, gfp);
    if (p) memcpy(p, src, len);
    return p;
}

extern struct sk_buff *alloc_skb(unsigned int size, gfp_t flags);
extern void dev_kfree_skb_any(struct sk_buff *skb);

static inline void usleep_range(unsigned long min, unsigned long max) { udelay(min); }

#define msecs_to_jiffies(m) ((m) * HZ / 1000)

#define static_assert _Static_assert

static inline bool is_multicast_ether_addr(const u8 *addr)
{
    return 0x01 & addr[0];
}

static inline bool ether_addr_equal(const u8 *a, const u8 *b)
{
    return a[0]==b[0] && a[1]==b[1] && a[2]==b[2] && a[3]==b[3] && a[4]==b[4] && a[5]==b[5];
}

#define WARN_ON(cond) WARN(cond, "WARN_ON")
#define WARN_ON_ONCE(cond) WARN_ON(cond)

#define clamp_t(type, val, lo, hi) min(max((type)(val), (type)(lo)), (type)(hi))

#define offsetofend(type, member) (offsetof(type, member) + sizeof(((type *)0)->member))

static inline unsigned long find_first_bit(const unsigned long *addr, unsigned long size)
{
    unsigned long i;
    for (i = 0; i < size; i++)
        if (test_bit(i, addr)) return i;
    return size;
}

static inline int atomic_inc_return(atomic_t *v) { return __sync_add_and_fetch(&v->counter, 1); }

static inline void reinit_completion(struct completion *c) { c->done = 0; }

static inline unsigned long wait_for_completion_timeout(struct completion *c, unsigned long timeout)
{
    unsigned long i;
    for (i = 0; i < timeout && !c->done; i++);
    return c->done ? timeout : 0;
}

#define skb_queue_walk_safe(queue, skb, tmp)                                    \
    for (skb = (queue)->next, tmp = skb->next;                                  \
         skb != (struct sk_buff *)(queue);                                      \
         skb = tmp, tmp = skb->next)

static inline void ieee80211_iter_keys(struct ieee80211_hw *hw,
    struct ieee80211_vif *vif,
    void (*iter)(struct ieee80211_hw *hw, struct ieee80211_vif *vif,
                 struct ieee80211_sta *sta, struct ieee80211_key_conf *key,
                 void *data), void *data) {}

static inline void ieee80211_iter_keys_rcu(struct ieee80211_hw *hw,
    struct ieee80211_vif *vif,
    void (*iter)(struct ieee80211_hw *hw, struct ieee80211_vif *vif,
                 struct ieee80211_sta *sta, struct ieee80211_key_conf *key,
                 void *data), void *data) {}

/* ── Additional missing types ── */

#define DECLARE_BITMAP(name, bits) unsigned long name[BITS_TO_LONGS(bits)]

struct ieee80211_key_conf {
    u32 key[4];
    u8 keylen;
    u32 cipher;
    u8 keyidx;
    u8 flags;
    u8 hw_key_idx;
};

struct ieee80211_tx_queue_params {
    u16 txop;
    u16 cw_min;
    u16 cw_max;
    u8 aifs;
    u8 _pad[7];
};

enum nl80211_dfs_regions {
    NL80211_DFS_UNSET = 0,
    NL80211_DFS_FCC   = 1,
    NL80211_DFS_ETSI  = 2,
    NL80211_DFS_JP    = 3,
};

enum led_brightness {
    LED_OFF  = 0,
    LED_HALF = 127,
    LED_FULL = 255,
};

struct led_classdev {
    const char *name;
    enum led_brightness brightness;
    enum led_brightness max_brightness;
    void (*brightness_set)(struct led_classdev *led, enum led_brightness brightness);
};

static inline int led_classdev_register(struct device *dev, struct led_classdev *led) { return 0; }
static inline void led_classdev_unregister(struct led_classdev *led) {}

struct cfg80211_sched_scan_plan {
    u32 _data[8];
};

typedef struct { unsigned long wait; } wait_queue_head_t;

extern void init_waitqueue_head(wait_queue_head_t *wq);

#define __nonstring

#define lockdep_assert_held(l) ((void)(l))

static inline unsigned long __ffs(unsigned long word) { return __builtin_ctzl(word); }
static inline unsigned long __ffsll(unsigned long long word) { return __builtin_ctzll(word); }
static inline unsigned long __fls(unsigned long word) { return word ? 8 * sizeof(word) - __builtin_clzl(word) - 1 : 0; }

static inline unsigned long find_first_zero_bit(const unsigned long *addr, unsigned long size)
{
    unsigned long i;
    for (i = 0; i < size; i++)
        if (!test_bit(i, addr)) return i;
    return size;
}

static inline void set_bit(unsigned int nr, volatile unsigned long *addr)
{
    __sync_fetch_and_or(addr, BIT(nr));
}

static inline void clear_bit(unsigned int nr, volatile unsigned long *addr)
{
    __sync_fetch_and_and(addr, ~BIT(nr));
}

#define rcu_read_lock() ((void)0)
#define rcu_read_unlock() ((void)0)

#define IEEE80211_VHT_CAP_SU_BEAMFORMER_CAPABLE        BIT(0)
#define IEEE80211_VHT_CAP_SU_BEAMFORMEE_CAPABLE        BIT(1)
#define IEEE80211_VHT_CAP_MU_BEAMFORMER_CAPABLE        BIT(2)
#define IEEE80211_VHT_CAP_MU_BEAMFORMEE_CAPABLE        BIT(3)
#define IEEE80211_VHT_CAP_SOUNDING_DIMENSIONS_MASK     (7 << 8)
#define IEEE80211_VHT_CAP_SOUNDING_DIMENSIONS_SHIFT    8
#define IEEE80211_VHT_CAP_MAX_MPDU_LENGTH_3895         0
#define IEEE80211_VHT_CAP_MAX_MPDU_LENGTH_7991         BIT(2)
#define IEEE80211_VHT_CAP_MAX_MPDU_LENGTH_11454        BIT(1)

static inline unsigned int hweight8(u8 w) { return __builtin_popcount(w); }
static inline unsigned int hweight16(u16 w) { return __builtin_popcount(w); }
static inline unsigned int hweight32(u32 w) { return __builtin_popcount(w); }

/* ── More missing types/constants ── */

struct ieee80211_rate {
    u32 bitrate;
    u16 hw_value;
    u16 hw_value_short;
    unsigned int flags;
    int beacon_rates;
};

struct ieee80211_iface_limit {
    u16 max;
    u16 types;
};

struct ieee80211_iface_combination {
    u32 max_interfaces;
    u16 num_different_channels;
    u8 _pad[18];
    const struct ieee80211_iface_limit *limits;
    u32 n_limits;
    u8 _pad2[4];
};

#define IEEE80211_IFACE_ITER_NORMAL 0

#define IEEE80211_KEY_FLAG_PAIRWISE BIT(0)
#define IEEE80211_KEY_FLAG_GENERATE_IV BIT(1)
#define IEEE80211_KEY_FLAG_GENERATE_MMIC BIT(2)
#define IEEE80211_KEY_FLAG_PUT_IV_SPACE BIT(3)

#define IEEE80211_CHAN_NO_HT40MINUS BIT(0)
#define IEEE80211_CHAN_NO_HT40PLUS  BIT(1)
#define IEEE80211_CHAN_NO_80MHZ     BIT(2)
#define IEEE80211_CHAN_NO_160MHZ    BIT(3)
#define IEEE80211_CHAN_NO_IR        BIT(4)
/* IEEE80211_CHAN_NO_IBSS and IEEE80211_CHAN_RADAR defined in regd.h */

#define IEEE80211_CONF_CHANGE_IDLE         BIT(0)
#define IEEE80211_CONF_CHANGE_CHANNEL      BIT(1)

#define U8_MAX  255
#define S8_MIN  (-128)

#define fallthrough __attribute__((__fallthrough__))

struct cfg80211_sar_freq_ranges {
    u32 start_freq;
    u32 end_freq;
};

#define NL80211_SAR_TYPE_POWER 0

struct cfg80211_sar_specs {
    u32 type;
    u32 num_sub_specs;
    struct {
        u32 freq_range_index;
        s32 power;
    } sub_specs[];
};

struct cfg80211_sar_capa {
    u32 type;
    u32 num_freq_ranges;
    const struct cfg80211_sar_freq_ranges *freq_ranges;
};

struct workqueue_struct;

struct regulatory_request {
    u32 initiator;
    u32 dfs_region;
    char alpha2[2];
};

static inline int queue_work(struct workqueue_struct *wq, struct work_struct *work) { return schedule_work(work); }
static inline bool cancel_work_sync(struct work_struct *work) { return flush_work(work); }
static inline bool cancel_delayed_work_sync(struct delayed_work *dwork) { return true; }

#define min_t(type, a, b) ({ type __a = (a); type __b = (b); __a < __b ? __a : __b; })
#define max_t(type, a, b) ({ type __a = (a); type __b = (b); __a > __b ? __a : __b; })

static inline int test_and_clear_bit(unsigned int nr, volatile unsigned long *addr)
{
    return __sync_fetch_and_and(addr, ~BIT(nr)) & BIT(nr);
}

static inline unsigned long find_next_zero_bit(const unsigned long *addr, unsigned long size, unsigned long offset)
{
    unsigned long i;
    for (i = offset; i < size; i++)
        if (!test_bit(i, addr)) return i;
    return size;
}

static inline void eth_broadcast_addr(u8 *addr)
{
    memset(addr, 0xff, ETH_ALEN);
}

static inline void eth_zero_addr(u8 *addr)
{
    memset(addr, 0, ETH_ALEN);
}

static inline unsigned long find_next_bit(const unsigned long *addr, unsigned long size, unsigned long offset)
{
    unsigned long i;
    for (i = offset; i < size; i++)
        if (test_bit(i, addr)) return i;
    return size;
}

#define wake_up(x) ((void)0)
#define wait_event_timeout(wq, cond, timeout) ({ unsigned long __t = (timeout); __t; })

static inline int skb_queue_empty(const struct sk_buff_head *list) { return list->qlen == 0; }

static inline u64 le64_get_bits(__le64 val, u64 mask)
{
    return (val >> (__builtin_ffsll(mask) - 1)) & (mask >> (__builtin_ffsll(mask) - 1));
}

static inline u32 u32_get_bits(u32 val, u32 mask)
{
    return (val >> (__builtin_ffs(mask) - 1)) & (mask >> (__builtin_ffs(mask) - 1));
}

#define kmalloc_obj(p, gfp) kmalloc(sizeof(p), gfp)

#define bitmap_zero(addr, nbits) memset(addr, 0, BITS_TO_LONGS(nbits) * sizeof(unsigned long))

extern struct sk_buff *skb_unlink(struct sk_buff *skb, struct sk_buff_head *list);
extern struct sk_buff *__skb_unlink(struct sk_buff *skb, struct sk_buff_head *list);

static inline unsigned char *skb_put_zero(struct sk_buff *skb, unsigned int len)
{
    unsigned char *p = skb_put(skb, len);
    memset(p, 0, len);
    return p;
}

static inline unsigned int bcd2bin(unsigned int val) { return (val & 0xf) + ((val >> 4) * 10); }

#define IEEE80211_CHAN_NO_IR        BIT(4)
#define IEEE80211_CHAN_RADAR        BIT(6)

#define IEEE80211_VIF_BEACON_FILTER BIT(0)

#define SIGNAL_DBM                  4
#define RX_INCLUDES_FCS             1
#define AMPDU_AGGREGATION           7
#define MFP_CAPABLE                 11
#define REPORTS_TX_ACK_STATUS       17
#define SUPPORTS_PS                 8
#define SUPPORTS_DYNAMIC_PS         10
#define SUPPORT_FAST_XMIT           16
#define SUPPORTS_AMSDU_IN_AMPDU     31
#define HAS_RATE_CONTROL            0
#define TX_AMSDU                    36
#define SINGLE_SCAN_ON_ALL_BANDS    29

#define IEEE80211_HT_CAP_SUP_WIDTH_20_40     BIT(0)
#define IEEE80211_HT_CAP_DSSSCCK40           BIT(11)
#define IEEE80211_HT_MAX_AMPDU_64K           0
#define IEEE80211_HT_MPDU_DENSITY_2          2
#define IEEE80211_HT_MCS_TX_DEFINED          BIT(0)

#define IEEE80211_VHT_CAP_RXSTBC_1           (1 << 8)
#define IEEE80211_VHT_CAP_HTC_VHT            BIT(5)
#define IEEE80211_VHT_CAP_TXSTBC             BIT(4)
#define IEEE80211_VHT_CAP_BEAMFORMEE_STS_SHIFT 13
#define IEEE80211_VHT_CAP_MAX_A_MPDU_LENGTH_EXPONENT_MASK (7 << 26)
#define IEEE80211_VHT_MCS_NOT_SUPPORTED       0x3

#define IEEE80211_MAX_DATA_LEN                2304

#define WQ_UNBOUND                            BIT(0)
#define WQ_HIGHPRI                            BIT(1)

#define WIPHY_FLAG_SUPPORTS_TDLS              BIT(0)
#define WIPHY_FLAG_TDLS_EXTERNAL_SETUP        BIT(1)

#define NL80211_FEATURE_SCAN_RANDOM_MAC_ADDR  BIT(0)
#define NL80211_EXT_FEATURE_CAN_REPLACE_PTK0  BIT(0)
#define NL80211_EXT_FEATURE_SCAN_RANDOM_SN    BIT(1)
#define NL80211_EXT_FEATURE_SET_SCAN_DWELL    BIT(2)

#define NL80211_SCAN_FLAG_RANDOM_ADDR         BIT(0)

#define IEEE80211_AMPDU_TX_START              0
#define IEEE80211_AMPDU_TX_STOP_CONT          1
#define IEEE80211_AMPDU_TX_STOP_FLUSH         2
#define IEEE80211_AMPDU_TX_STOP_FLUSH_CONT    3
#define IEEE80211_AMPDU_TX_OPERATIONAL        4
#define IEEE80211_AMPDU_RX_START              5
#define IEEE80211_AMPDU_RX_STOP               6
#define IEEE80211_AMPDU_TX_START_IMMEDIATE    7

#define IEEE80211_RC_BW_CHANGED               BIT(0)

#define NL80211_STA_INFO_TX_BITRATE           BIT(0)
#define IEEE80211_VIF_SUPPORTS_CQM_RSSI BIT(1)

struct cfg80211_ssid {
    u8 ssid[IEEE80211_MAX_SSID_LEN];
    u8 ssid_len;
};

struct cfg80211_sched_scan_request {
    u32 _data[64];
};

struct cfg80211_pno_request {
    u32 _data[64];
    struct cfg80211_match_set *match_sets;
    int n_match_sets;
};

#define IEEE80211_SRVCC_NO_SRVCC 0

struct ieee80211_txq_params {
    u16 txop;
    u16 cw_min;
    u16 cw_max;
    u8 aifs;
    bool uapsd;
};

#define wiphy_regulatory_flags(w) ((w)->regulatory_flags)

#define ETH_P_PAE 0x888E

#define FIF_ALLMULTI              BIT(0)
#define FIF_FCSFAIL               BIT(1)
#define FIF_OTHER_BSS             BIT(2)
#define FIF_BCN_PRBRESP_PROMISC   BIT(3)

#define REGULATORY_STRICT_REG          BIT(0)
#define REGULATORY_COUNTRY_IE_IGNORE   BIT(1)

extern int regulatory_hint(struct wiphy *wiphy, const char *alpha2);

struct cfg80211_bitrate_mask {
    struct {
        u32 legacy;
        u8 ht_mcs[4];
        u16 vht_mcs[2];
    } control[3];
};

static inline u64 u64_encode_bits(u64 val, u64 mask)
{
    return (val << (__builtin_ffsll(mask) - 1)) & mask;
}

extern struct sk_buff *skb_put_data(struct sk_buff *skb, const void *data, unsigned int len);

/* ── Shim function declarations (implemented in symbols.rs) ── */

extern int ieee80211_emulate_add_chanctx(struct ieee80211_hw *hw, void *ctx);
extern void ieee80211_emulate_remove_chanctx(struct ieee80211_hw *hw, void *ctx);
extern void ieee80211_emulate_change_chanctx(struct ieee80211_hw *hw, void *ctx, u32 changed);
extern int ieee80211_emulate_switch_vif_chanctx(struct ieee80211_hw *hw, void *vifs, int n_vifs, void *old_ctx, void *new_ctx);

/* ── HT/VHT capability constants ── */

#define IEEE80211_HT_CAP_LDPC_CODING         BIT(0)
#define IEEE80211_HT_CAP_MAX_AMSDU           BIT(1)
#define IEEE80211_HT_CAP_SGI_20              BIT(2)
#define IEEE80211_HT_CAP_SGI_40              BIT(3)
#define IEEE80211_HT_CAP_TX_STBC             BIT(4)
#define IEEE80211_HT_CAP_RX_STBC             (3 << 5)
#define IEEE80211_HT_CAP_RX_STBC_SHIFT       5

#define IEEE80211_VHT_CAP_RXLDPC             BIT(1)
#define IEEE80211_VHT_CAP_SHORT_GI_80        BIT(3)
#define IEEE80211_VHT_CAP_RXSTBC_MASK        (7 << 8)

#define IEEE80211_STA_RX_BW_20               0
#define IEEE80211_STA_RX_BW_40               1
#define IEEE80211_STA_RX_BW_80               2

#define ENOTSUPP                             524

#define NUM_NL80211_BANDS                    4

#define NL80211_SCAN_FLAG_RANDOM_SN          BIT(0)

#define BSS_CHANGED_ASSOC                    BIT(0)
#define BSS_CHANGED_BEACON_INT               BIT(1)
#define BSS_CHANGED_BSSID                    BIT(2)
#define BSS_CHANGED_BEACON                   BIT(3)
#define BSS_CHANGED_BEACON_ENABLED           BIT(4)
#define BSS_CHANGED_CQM                      BIT(5)
#define BSS_CHANGED_MU_GROUPS                BIT(6)
#define BSS_CHANGED_ERP_SLOT                 BIT(7)
#define BSS_CHANGED_PS                       BIT(8)

#define IEEE80211_KEY_FLAG_SW_MGMT_TX        BIT(4)

#define WLAN_CIPHER_SUITE_AES_CMAC           0x000FAC06
#define WLAN_CIPHER_SUITE_BIP_CMAC_256       0x000FAC0D
#define WLAN_CIPHER_SUITE_BIP_GMAC_128       0x000FAC0E
#define WLAN_CIPHER_SUITE_BIP_GMAC_256       0x000FAC0F
#define WLAN_CIPHER_SUITE_CCMP_256           0x000FAC10
#define WLAN_CIPHER_SUITE_GCMP               0x000FAC08
#define WLAN_CIPHER_SUITE_GCMP_256           0x000FAC09

#define NL80211_REGDOM_SET_BY_USER           0
#define NL80211_REGDOM_SET_BY_DRIVER         1

/* ── Additional struct fields ── */

struct ieee80211_scan_ies {
    u8 *ies[3];
    size_t len[3];
    u8 *common_ies;
    size_t common_ie_len;
};

struct cfg80211_scan_request {
    struct cfg80211_ssid *ssids;
    int n_ssids;
    u32 n_channels;
    struct ieee80211_channel **channels;
    unsigned long flags;
    u16 duration;
    bool duration_mandatory;
    bool no_cck;
    u8 *mac_addr;
    u8 *mac_addr_mask;
    u32 ie_len;
    u32 n_6ghz_params;
};

struct cfg80211_scan_info {
    u64 scan_start_tsf;
    bool aborted;
};

struct ieee80211_scan_request {
    struct cfg80211_scan_request req;
    struct ieee80211_scan_ies ies;
};

enum set_key_cmd {
    SET_KEY = 0,
    DISABLE_KEY = 1,
};

enum ieee80211_reconfig_type {
    IEEE80211_RECONFIG_TYPE_RESTART = 0,
};

struct ieee80211_ampdu_params {
    struct ieee80211_sta *sta;
    u16 tid;
    u8 *ssn;
    u16 buf_size;
    u16 timeout;
    u16 amsdu;
    u16 ssn_valid;
    u8 action;
};

struct station_info {
    u64 filled;
    u32 _pad[16];
    struct rate_info txrate;
};

/* ── DMA constants ── */
#define DMA_TO_DEVICE 1
#define DMA_FROM_DEVICE 2
#define DMA_BIDIRECTIONAL 3

/* ── DMA function declarations (implemented in symbols.rs) ── */
extern void *dma_alloc_coherent(struct device *dev, size_t size, dma_addr_t *dma_addr, gfp_t gfp);
extern void dma_free_coherent(struct device *dev, size_t size, void *cpu_addr, dma_addr_t dma_addr);
extern dma_addr_t dma_map_single(struct device *dev, void *cpu_addr, size_t size, int dir);
extern void dma_unmap_single(struct device *dev, dma_addr_t dma_addr, size_t size, int dir);
extern void dma_sync_single_for_device(struct device *dev, dma_addr_t dma_addr, size_t size, int dir);
extern void dma_sync_single_for_cpu(struct device *dev, dma_addr_t dma_addr, size_t size, int dir);

/* ── PCI helpers ── */
#define to_pci_dev(d) container_of(d, struct pci_dev, dev)

/* ── skb helpers ── */
extern struct sk_buff *skb_copy(const struct sk_buff *skb, gfp_t gfp);
static inline void dev_kfree_skb(struct sk_buff *skb) { dev_kfree_skb_any(skb); }

/* ── Missing kernel helpers ── */
#define kzalloc_obj(P, ...) kzalloc(sizeof(typeof(P)), GFP_KERNEL)

#define u8p_replace_bits(p, v, m) ({ u8 __v = (*(p) & ~(m)) | ((v) & (m)); *(p) = __v; })
#define u32p_replace_bits(p, v, m) ({ u32 __v = (*(p) & ~(m)) | ((v) & (m)); *(p) = __v; })

extern void get_random_mask_addr(u8 *addr, const u8 *mask, const u8 *addr2);

#define ilog2(x) (__builtin_constant_p(x) ? (8 * sizeof(x) - __builtin_clzll(x) - 1) : 31)

struct ieee80211_prep_tx_info {
    struct ieee80211_sta *sta;
    u8 link_id;
};

extern void *devm_kmemdup(struct device *dev, const void *src, size_t len, gfp_t gfp);
#define devm_kmemdup_array(dev, src, n, size, gfp) devm_kmemdup(dev, src, (n) * (size), gfp)

extern void complete_all(struct completion *c);

extern int request_firmware_nowait(struct module *mod, bool uevent, const char *name,
                                   struct device *dev, gfp_t gfp, void *ctx,
                                   void (*cb)(const struct firmware *fw, void *ctx));

static inline bool is_valid_ether_addr(const u8 *addr)
{
    return !(addr[0] & 1) && !(addr[0] == 0 && addr[1] == 0 && addr[2] == 0 &&
                                addr[3] == 0 && addr[4] == 0 && addr[5] == 0);
}
static inline void eth_random_addr(u8 *addr)
{
    addr[0] = 0x02; addr[1] = 0x00; addr[2] = 0x00;
    addr[3] = 0x00; addr[4] = 0x00; addr[5] = 0x00;
}
static inline bool is_zero_ether_addr(const u8 *addr)
{
    return addr[0] == 0 && addr[1] == 0 && addr[2] == 0 &&
           addr[3] == 0 && addr[4] == 0 && addr[5] == 0;
}

#define timer_setup(t, cb, flags) do { (t)->function = (cb); (t)->data = 0; } while(0)

extern struct workqueue_struct *alloc_workqueue(const char *name, unsigned int flags, int max_active);
extern void destroy_workqueue(struct workqueue_struct *wq);

#define INIT_DELAYED_WORK(w, f) do { (w)->work.func = (void*)(f); } while(0)

extern bool timer_delete_sync(struct timer_list *t);

static inline void mutex_destroy(struct mutex *m) {}

static inline void wiphy_ext_feature_set(struct wiphy *wiphy, unsigned int feature) {}

#define SET_IEEE80211_PERM_ADDR(hw, addr) ((void)(hw), (void)(addr))

#define min3(a, b, c) min(min(a, b), c)
#define max3(a, b, c) max(max(a, b), c)

static inline void fsleep(unsigned long usecs) { udelay(usecs); }

static inline const char *dev_name(const struct device *dev) { return "rtw88"; }

#define LED_ON 1

struct ieee80211_tpt_blink { int _data; };

static inline int dma_mapping_error(struct device *dev, dma_addr_t dma_addr) { return 0; }

static inline int test_and_set_bit(unsigned int nr, volatile unsigned long *addr)
{
    return __sync_fetch_and_or(addr, BIT(nr)) & BIT(nr);
}

extern void napi_synchronize(struct napi_struct *napi);

static inline void dev_kfree_skb_irq(struct sk_buff *skb) { dev_kfree_skb_any(skb); }

extern unsigned char *skb_pull(struct sk_buff *skb, unsigned int len);

/* ── TX flags ── */
#define IEEE80211_TX_CTL_NO_ACK 0x00000008
#define IEEE80211_TX_STAT_NOACK_TRANSMITTED 0x00000800

/* ── NAPI ── */
extern void napi_schedule(struct napi_struct *napi);
extern bool napi_complete_done(struct napi_struct *napi, int work_done);

/* ── irqreturn_t ── */
typedef int irqreturn_t;
#define IRQ_WAKE_THREAD 2

/* ── PCI iomap ── */
extern void *pci_iomap(struct pci_dev *dev, int bar, unsigned long maxlen);
extern void pci_iounmap(struct pci_dev *dev, void *addr);
extern int pci_alloc_irq_vectors(struct pci_dev *dev, unsigned int min_vecs, unsigned int max_vecs, unsigned int flags);
extern void pci_free_irq_vectors(struct pci_dev *dev);

#define PCI_IRQ_INTX 0x00000001
#define PCI_IRQ_MSI  0x00000002

/* ── PCI Express constants ── */
#define PCI_EXP_LNKCTL          16
#define PCI_EXP_LNKCTL_CLKREQ_EN 0x0100
#define PCI_EXP_LNKCTL_ASPM_L1  0x0002
#define PCI_EXP_DEVCTL2         40
#define PCI_EXP_DEVCTL2_COMP_TMOUT_DIS 0x0010

/* ── PCIe capability helpers ── */
extern int pcie_capability_read_word(struct pci_dev *dev, int pos, u16 *val);
extern int pcie_capability_set_word(struct pci_dev *dev, int pos, u16 set);

/* ── atomic helpers ── */
static inline int atomic_dec_if_positive(atomic_t *v)
{
    int c = __sync_fetch_and_sub(&v->counter, 1);
    if (c <= 0) { __sync_fetch_and_add(&v->counter, 1); return -1; }
    return c - 1;
}

/* ── device helpers ── */
static inline void *dev_get_drvdata(const struct device *dev) { return dev->driver_data; }
static inline void dev_set_drvdata(struct device *dev, void *data) { ((struct device *)dev)->driver_data = data; }

/* ── WARN_ONCE ── */
#define WARN_ONCE(cond, fmt, ...) ({ if (cond) printk(fmt, ##__VA_ARGS__); 0; })

/* ── PM ops ── */
#define SIMPLE_DEV_PM_OPS(name, suspend_fn, resume_fn) \
    const struct dev_pm_ops name = { }

/* ── IEEE80211 device helpers ── */
#define SET_IEEE80211_DEV(hw, dev) ((void)(hw), (void)(dev))

/* ── devm IRQ helpers ── */
extern int devm_request_threaded_irq(struct device *dev, unsigned int irq,
    void *handler, void *thread_fn, unsigned long flags, const char *name, void *dev_id);
extern void devm_free_irq(struct device *dev, unsigned int irq, void *dev_id);

/* ── Netdev dummy ── */
extern struct net_device *alloc_netdev_dummy(int sizeof_priv);

/* ── PCI error handling types ── */
typedef int pci_ers_result_t;
typedef int pci_channel_state_t;
#define PCI_ERS_RESULT_RECOVERED 1
#define PCI_D0 0

extern int pci_enable_wake(struct pci_dev *pdev, int state, bool enable);
extern struct pci_dev *pci_upstream_bridge(struct pci_dev *pdev);
extern int pci_set_power_state(struct pci_dev *pdev, int state);

#define PCI_VENDOR_ID_INTEL 0x8086
#define PCI_D3hot 3
#define PCI_ERS_RESULT_NEED_RESET 2

extern void netif_device_detach(struct net_device *dev);
extern void netif_device_attach(struct net_device *dev);

#endif /* __MESAOS_COMPAT_H */
