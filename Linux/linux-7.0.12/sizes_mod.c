#include <linux/module.h>
#include <linux/kernel.h>
#include <net/mac80211.h>
#include <net/cfg80211.h>

static int __init sizes_init(void)
{
    printk("SIZES: sizeof(ieee80211_hw)=%zu sizeof(conf)=%zu sizeof(rx_status)=%zu sizeof(tx_info)=%zu sizeof(vif)=%zu sizeof(sta)=%zu sizeof(key_conf)=%zu sizeof(wiphy)=%zu sizeof(channel)=%zu sizeof(supported_band)=%zu", 
        sizeof(struct ieee80211_hw), sizeof(struct ieee80211_conf), sizeof(struct ieee80211_rx_status),
        sizeof(struct ieee80211_tx_info), sizeof(struct ieee80211_vif), sizeof(struct ieee80211_sta),
        sizeof(struct ieee80211_key_conf), sizeof(struct wiphy), sizeof(struct ieee80211_channel),
        sizeof(struct ieee80211_supported_band));
    printk("SIZES: hw.priv=%zu hw.wiphy=%zu hw.flags=%zu hw.vif_data_size=%zu hw.sta_data_size=%zu hw.queues=%zu hw.max_signal=%zu",
        offsetof(struct ieee80211_hw, priv), offsetof(struct ieee80211_hw, wiphy),
        offsetof(struct ieee80211_hw, flags), offsetof(struct ieee80211_hw, vif_data_size),
        offsetof(struct ieee80211_hw, sta_data_size), offsetof(struct ieee80211_hw, queues),
        offsetof(struct ieee80211_hw, max_signal));
    printk("SIZES: vif.type=%zu vif.bss_conf=%zu vif.drv_priv=%zu sta.drv_priv=%zu wiphy.priv=%zu",
        offsetof(struct ieee80211_vif, type), offsetof(struct ieee80211_vif, bss_conf),
        offsetof(struct ieee80211_vif, drv_priv), offsetof(struct ieee80211_sta, drv_priv),
        offsetof(struct wiphy, priv));
    printk("SIZES: nl80211_band_2ghz=%zu nl80211_band_5ghz=%zu",
        (size_t)NL80211_BAND_2GHZ, (size_t)NL80211_BAND_5GHZ);
    return -1;
}
module_init(sizes_init);
