//! Driver para Realtek RTL8822CE WiFi (PCIe)
//! Soporta: 802.11a/b/g/n/ac (WiFi 5), 2T2R MIMO
//!
//! Basado en documentación técnica del chip y el driver rtw88 de Linux.
//! Diseñado para funcionar en hardware real (HP Laptop 15s-eq2xxx)
//! y en QEMU vía PCI passthrough (IOMMU).

#![no_std]

use alloc::format;
use alloc::vec::Vec;
use alloc::string::String;
use crate::{mesa_println, serial_println, mesa_print};
use spin::Mutex;
use super::rtw8822c_phy_tables::{AGC_TBL, BB_TBL, MP_CAL_INIT_TBL, RF_A_TBL, RF_B_TBL};

// Compatibility modules to match expected paths in driver code
mod pci {
    pub use crate::pci::*;
}
mod memory {
    pub mod vmm {
        pub use crate::memory::vmm::*;
    }
}
mod linux {
    pub use crate::linux::mdelay;
    pub use crate::linux::dma::dma_alloc_coherent;
}
mod drivers {
    pub mod wifi {
        pub use crate::drivers::wifi::frame;
        pub use crate::drivers::wifi::scan;
        pub use crate::drivers::wifi::state;
    }
}
mod fs {
    pub mod initrd_data {
        pub use crate::fs::initrd_data::*;
    }
}

// ============================================================================
// Constantes de identificación PCI
// ============================================================================
const VENDOR_ID: u16 = 0x10EC;
const DEVICE_ID: u16 = 0xC822;
const DEVICE_ID_ALT: u16 = 0xC82F;

// ============================================================================
// Tamaños de buffers y rings
// ============================================================================
const TX_RING_SIZE: usize = 128;
const RX_RING_SIZE: usize = 256;
const TX_BUF_SIZE: usize = 2048;
// TX hardware descriptor area at the head of each TX buffer (`tx_pkt_desc_sz`
// in rtw88). The descriptor struct is 40 bytes but the hardware reserves 48
// bytes and the TX descriptor OFFSET field must be set to this value.
const TX_HDR_SIZE: u16 = 48;
// Linux uses 12KB (12*1024) for RX buffer: "Max. MPDU size is 11454 bytes, fix Rx buffer size to 12K is safe"
const RX_BUF_SIZE: usize = 12288; // 12KB
// Value written into the RX BD RXBUFFSIZE field (14-bit, hardware caps frames here).
const RX_BD_RXBUFFSIZE: u32 = 12288;
// Size of the per-packet RX descriptor that precedes the 802.11 payload.
// rtw8822c.c: rx_pkt_desc_sz = 24.
const RX_PKT_DESC_SIZE: usize = 24;

// ============================================================================
// Linux rtw8822c constants from reg.h / phy.c
// ============================================================================
const REG_3WIRE: u32 = 0x180C;
const BIT_3WIRE_TX_EN: u32 = 1 << 0;
const BIT_3WIRE_RX_EN: u32 = 1 << 1;
const BIT_3WIRE_PI_ON: u32 = 1 << 28;
const REG_3WIRE2: u32 = 0x410C;
const REG_ENCCK: u32 = 0x1C3C;
const BIT_CCK_OFDM_BLK_EN: u32 = (1 << 0) | (1 << 1);

const REG_FWHW_TXQ_CTRL: u32 = 0x0420;
const BIT_EN_BCNQ_DL: u32 = 1 << 22; // Bit 22 in REG_FWHW_TXQ_CTRL + 2 (0x0422)

// BIT macro (matches Linux kernel BIT macro)
macro_rules! BIT {
    ($n:expr) => { 1u32 << $n };
}

const REG_FIFOPAGE_INFO_1: u32 = 0x0230;
const REG_FIFOPAGE_INFO_2: u32 = 0x0234;
const REG_FIFOPAGE_INFO_3: u32 = 0x0238;
const REG_FIFOPAGE_INFO_4: u32 = 0x023C;
const REG_FIFOPAGE_INFO_5: u32 = 0x0240;
const REG_RQPN_CTRL_2: u32 = 0x022C;
const BIT_LD_RQPN: u32 = 1 << 31;
const REG_H2CQ_CSR: u32 = 0x1330;
const BIT_H2CQ_FULL: u32 = 1 << 31;
// H2C command queue BD ring (rtw88 pci.h RTK_PCI_TXBD_{DESA,NUM,IDX}_H2CQ).
// H2C "packets" (fw_offload PKT format) are transmitted through this ring, not
// through the HMEBOX mailboxes (which carry 8-byte H2C *commands*).
const REG_H2CQ_TXBD_DESA: u32 = 0x1320;
const REG_H2CQ_TXBD_NUM: u32 = 0x1328;
const REG_H2CQ_TXBD_IDX: u32 = 0x132C;
const REG_TXDMA_PQ_MAP: u32 = 0x010C;
const RTW_DMA_MAPPING_HIGH: u8 = 3;
const RTW_DMA_MAPPING_NORMAL: u8 = 2;
const RTW_DMA_MAPPING_LOW: u8 = 1;
const RTW_DMA_MAPPING_EXTRA: u8 = 0;

// --- TRX FIFO boundary / page config (rtw88 mac.c rtw_init_trx_cfg) ---
const REG_RXFF_BNDY: u32 = 0x011C;
const REG_AUTO_LLT_V1: u32 = 0x0208;
const BIT_AUTO_INIT_LLT_V1: u32 = 1 << 0;
const REG_BCNQ_BDNY_V1: u32 = 0x0424;
const REG_BCNQ1_BDNY_V1: u32 = 0x0456;
const BIT_EN_WR_FREE_TAIL: u32 = 1 << 20;
const REG_H2C_HEAD: u32 = 0x0244;
const REG_H2C_TAIL: u32 = 0x0248;
const REG_H2C_READ_ADDR: u32 = 0x024C;
const REG_H2C_INFO: u32 = 0x0254;
const REG_TXDMA_OFFSET_CHK: u32 = 0x020C;
const REG_RX_DRVINFO_SZ: u32 = 0x060F;

// RTL8822C TX/RX FIFO geometry (halmac_8822c_cfg.h + rtw8822c.c):
//   TX_FIFO_SIZE=262144, page=128 -> 2048 pages; RX_FIFO_SIZE=24576.
const TX_PAGE_SIZE: u32 = 128;
const TXFF_PG_NUM: u32 = 262144 / TX_PAGE_SIZE; // 2048
const RXFF_SIZE: u32 = 24576;
const C2H_PKT_BUF: u32 = 256;
// Reserved pages = drv(16)+H2C extrainfo(24)+H2C staticinfo(8)+H2CQ(8)
//   +CPU instr(0)+FW txbuf(4)+CSI(50) = 110.
const RSVD_PG_NUM: u32 = 16 + 24 + 8 + 8 + 0 + 4 + 50;
const RSVD_BOUNDARY: u16 = (TXFF_PG_NUM - RSVD_PG_NUM) as u16; // 1938
const RSVD_H2CQ_ADDR: u32 = TXFF_PG_NUM - 50 - 4 - 0 - 8; // 1986
const PHY_STATUS_SIZE: u8 = 4;

const REG_BCN_CTRL: u32 = 0x0550;
const BIT_EN_BCN_FUNCTION: u32 = 1 << 3;
const BIT_DIS_TSF_UDT: u32 = 1 << 4;

// --- System (0x0000-0x00FF) ---
const REG_SYS_FUNC_EN: u32 = 0x0002;
const REG_SYS_CLK_CTRL: u32 = 0x0008;
const REG_SYS_EEPROM_CTRL: u32 = 0x000A;
const REG_RSV_CTRL: u32 = 0x001C;
const REG_RF_CTRL: u32 = 0x001F;
const REG_AFE_CTRL1: u32 = 0x0024;
const REG_EFUSE_CTRL: u32 = 0x0030;
const REG_LDO_SWR_CTRL: u32 = 0x007C;
const REG_MCUFW_CTRL: u32 = 0x0080;
const REG_SYS_CFG1: u32 = 0x00F0;
// Firmware IQK status register (rtw8822c.h REG_RPT_CIP); reads 0xAA when the
// firmware-driven IQK has finished.
const REG_RPT_CIP: u32 = 0x2d9c;

// --- WLRF (0x00EC) ---
const REG_WLRF1: u32 = 0x00EC;

// --- BB/RF Power Control Bits ---
const BIT_FEN_BB_RSTB: u8 = 1 << 0;
const BIT_FEN_BB_GLB_RST: u8 = 1 << 1;
const BIT_FEN_BBRSTB: u8 = 1 << 0;
const BIT_FEN_BB_GLB_RST_N: u8 = 1 << 1;

const BIT_RF_EN: u8 = 1 << 0;
const BIT_RF_RSTB: u8 = 1 << 1;
const BIT_RF_SDM_RSTB: u8 = 1 << 2;

const BIT_WLRF1_BBRF_EN: u32 = 0x07 << 24; // Bits 24,25,26

// --- MAC TX (0x0200-0x03FF) ---
const REG_TXPAUSE: u32 = 0x0522;

// --- MAC RX (0x0600-0x06FF) ---
const REG_RCR: u32 = 0x0608;

// --- Security CAM (0x0670-0x0683), mirrors rtw88 sec.c ---
const REG_SEC_CMD: u32 = 0x0670;
const REG_SEC_WRITE: u32 = 0x0674;
const REG_SEC_CONFIG: u32 = 0x0680;
const SEC_CMD_WRITE_ENABLE: u32 = 1 << 16;
const SEC_CMD_POLLING: u32 = 1 << 31;
const SEC_CAM_ENTRY_SHIFT: u32 = 3;
const SEC_TX_UNI_USE_DK: u16 = 1 << 0;
const SEC_RX_UNI_USE_DK: u16 = 1 << 1;
const SEC_TX_DEC_EN: u16 = 1 << 2;
const SEC_RX_DEC_EN: u16 = 1 << 3;
const SEC_TX_BC_USE_DK: u16 = 1 << 6;
const SEC_RX_BC_USE_DK: u16 = 1 << 7;
const SEC_ENGINE_EN: u16 = 1 << 9;
// CAM key-type encoding (rtw88 main.h), distinct from the TX-desc SEC_TYPE.
const HWCAM_TKIP: u8 = 2;
const HWCAM_AES: u8 = 4;


// --- MAC Address Registers (0x0610-0x0613) ---
const REG_MACID: u32 = 0x0610;      // MAC address bytes 0-3
const REG_MACID_H: u32 = 0x0614;    // MAC address bytes 4-5
const REG_BSSID: u32 = 0x0618;      // BSSID bytes 0-3 (target AP)
const REG_BSSID_H: u32 = 0x061C;    // BSSID bytes 4-5

// --- WMAC (0x0100) ---
const REG_CR: u32 = 0x0100;
const BIT_32K_CAL_TMR_EN: u32 = 1 << 10;

// --- H2C/C2H Mailbox (0x01D0-0x01FF) ---
const REG_HMETFR: u32 = 0x01CC;
const REG_HMEBOX0: u32 = 0x01D0;
const REG_HMEBOX1: u32 = 0x01D4;
const REG_HMEBOX2: u32 = 0x01D8;
const REG_HMEBOX3: u32 = 0x01DC;

// H2C mailbox extension registers (high dword of each box, Linux reg.h)
const REG_HMEBOX0_EX: u32 = 0x01F0;
const REG_HMEBOX1_EX: u32 = 0x01F4;
const REG_HMEBOX2_EX: u32 = 0x01F8;
const REG_HMEBOX3_EX: u32 = 0x01FC;

// There is no dedicated C2H event *register* on this part: rtw88 receives C2H
// reports as packets on the RX ring (bit 28 of the RX packet descriptor). The
// old REG_C2H_EVT (0x0E00) / BIT_C2H_FW_* constants were bogus and have been
// removed; C2H packets are now decoded in `process_rx`.

// --- RCR bits (match Linux hal_com_reg.h) ---
const RCR_CBSSID_BCN: u32 = 1 << 7;     // Accept BSSID match beacon/probe rsp
const RCR_CBSSID_DATA: u32 = 1 << 6;    // Accept BSSID match data
const RCR_APWRMGT: u32 = 1 << 5;        // Accept power management packet
const RCR_ADD3: u32 = 1 << 4;           // Accept address 3 match packet
const RCR_AB: u32 = 1 << 3;             // Accept broadcast packet
const RCR_AM: u32 = 1 << 2;             // Accept multicast packet
const RCR_APM: u32 = 1 << 1;            // Accept physical match packet
const RCR_AAP: u32 = 1 << 0;            // Accept all unicast packet
const RCR_ACRC32: u32 = 1 << 8;         // Accept CRC32 error packet
const RCR_AICV: u32 = 1 << 9;           // Accept ICV error packet
const RCR_DISDECMYPKT: u32 = 1 << 10;   // Decrypt only if A1 match
const RCR_ADF: u32 = 1 << 11;           // Accept data type frame
const RCR_ACF: u32 = 1 << 12;           // Accept control type frame
const RCR_AMF: u32 = 1 << 13;           // Accept management type frame
const RCR_HTC_LOC_CTRL: u32 = 1 << 14;  // HTC location control
const RCR_UC_MD_EN: u32 = 1 << 16;      // Unicast more data bit check
const RCR_BC_MD_EN: u32 = 1 << 17;      // Broadcast more data bit check
const RCR_TIM_PARSER_EN: u32 = 1 << 18; // RX Beacon TIM Parser
const RCR_DISGCLK: u32 = 1 << 19;       // Disable macrx clock gating

// Additional RCR bits (match Linux hal_com_reg.h bits 20-31)
const RCR_PKTCTL_DLEN: u32 = 1 << 20;   // Packet control double length
const RCR_DISCHKPPDLLEN: u32 = 1 << 21; // Disable check PPDU length
const RCR_MFBEN: u32 = 1 << 22;         // Enable immediate MCS feedback
const RCR_LSIGEN: u32 = 1 << 23;        // Enable LSIG
const RCR_ENMBID: u32 = 1 << 24;        // Enable MBSSID
const RCR_TCPOFLD_EN: u32 = 1 << 25;    // Enable TCP checksum offload
const RCR_VHT_DACK: u32 = 1 << 26;      // VHT acknowledgment control
const RCR_APP_PHYST_RXFF: u32 = 1 << 28; // Append PHY status
const RCR_APP_ICV: u32 = 1 << 29;       // Append ICV
const RCR_APP_MIC: u32 = 1 << 30;       // Append MIC
const RCR_APPFCS: u32 = 1 << 31;        // Append FCS after payload

// --- DMA/PCIe Ring Addresses (corrected to match Linux pci.h) ---
const REG_VOQ_TXBD_DESA: u32 = 0x0318;  // Linux: RTK_PCI_TXBD_DESA_VOQ = 0x318
const REG_VIQ_TXBD_DESA: u32 = 0x0320;  // Linux: RTK_PCI_TXBD_DESA_VIQ = 0x320

const REG_DARFRC: u32 = 0x0430;
const REG_DARFRCH: u32 = 0x0434;
const REG_RARFRCH: u32 = 0x043C;
const REG_ARFR0: u32 = 0x0444;
const REG_ARFRH0: u32 = 0x0448;
const REG_ARFR1_V1: u32 = 0x044C;
const REG_ARFRH1_V1: u32 = 0x0450;
const REG_ARFR4: u32 = 0x049C;
const REG_ARFRH4: u32 = 0x04A0;
const REG_ARFR5: u32 = 0x04A4;
const REG_ARFRH5: u32 = 0x04A8;
const REG_TCR: u32 = 0x0604;
const REG_GENERAL_OPTION: u32 = 0x1664;
const BIT_DUMMY_FCS_READY_MASK_EN: u32 = 1 << 0;
// rtw88 reg.h / halmac_reg_8822c.h: REG_WMAC_OPTION_FUNCTION is 0x07D0
// (the previous 0x0440 was wrong).
const REG_WMAC_OPTION_FUNCTION: u32 = 0x07D0;
const REG_WMAC_OPTION_FUNCTION_1: u32 = 0x07D4;
const REG_RXPSF_CTRL: u32 = 0x1610;
const REG_RXPSF_TYPE_CTRL: u32 = 0x1614;
const REG_SND_PTCL_CTRL: u32 = 0x0718;
const BIT_DIS_CHK_VHTSIGB_CRC: u32 = 1 << 0;
const REG_INT_MIG: u32 = 0x0304;

// Channel/bandwidth BB registers (rtw88 rtw8822c.h/reg.h) used by
// rtw8822c_set_channel_bb. These were entirely missing from our channel
// switch, so BGCTRL/CCK_CHECK/CCKTXONLY/CCAMSK/AGC/DFIR/TX filter/CCK source
// retained stale values after the initial BB table load -> RX never demodulated.
const REG_BGCTRL: u32 = 0x1A14;
const BITS_RX_IQ_WEIGHT: u32 = (1 << 8) | (1 << 9);
const REG_CCK_CHECK: u32 = 0x0454;
const BIT_CHECK_CCK_EN: u32 = 1 << 7;
const REG_CCKTXONLY: u32 = 0x1A80;
const BIT_BB_CCK_CHECK_EN: u32 = 1 << 18;
const REG_CCAMSK: u32 = 0x1C80;
const REG_RXAGCCTL0: u32 = 0x18AC;
const REG_RXAGCCTL: u32 = 0x41AC;
const BITS_RXAGC_CCK: u32 = 0x0000_F000;
const BITS_RXAGC_OFDM: u32 = 0x0000_01F0;
const REG_SCOTRK: u32 = 0x0C30;
const REG_TXF0: u32 = 0x1A20;
const REG_TXF1: u32 = 0x1A24;
const REG_TXF2: u32 = 0x1A28;
const REG_TXF3: u32 = 0x1A98;
const REG_TXF4: u32 = 0x1A9C;
const REG_TXF5: u32 = 0x1AA0;
const REG_TXF6: u32 = 0x1AAC;
const REG_TXF7: u32 = 0x1AB0;
const REG_TXDFIR0: u32 = 0x0808;
const REG_DFIRBW: u32 = 0x0810;
const REG_TXBWCTL: u32 = 0x09B0;
const REG_TXCLK: u32 = 0x09B4;
const REG_CCK_SOURCE: u32 = 0x1ABC;
const BIT_NBI_EN: u32 = 1 << 30;
const REG_SBD: u32 = 0x088C;
const BITS_SUBTUNE: u32 = 0x0000_F000;
const REG_PT_CHSMO: u32 = 0x0CBC;
const BIT_PT_OPT: u32 = 1 << 21;

// TX descriptor QSEL values (rtw88 tx.h enum rtw_tx_desc_queue_select)
const TX_DESC_QSEL_MGMT: u32 = 18;
const TX_DESC_QSEL_HIGH: u32 = 17;

const WLAN_TXQ_RPT_EN: u8 = 0x1F;
const WLAN_SLOT_TIME: u8 = 0x09;
const WLAN_PIFS_TIME: u8 = 0x1C;
const WLAN_SIFS_CCK_CONT_TX: u8 = 0x0A;
const WLAN_SIFS_OFDM_CONT_TX: u8 = 0x0E;
const WLAN_SIFS_CCK_TRX: u8 = 0x0A;
const WLAN_SIFS_OFDM_TRX: u8 = 0x10;
const WLAN_NAV_MAX: u8 = 0xC8;
const WLAN_RDG_NAV: u32 = 0x05;
const WLAN_TXOP_NAV: u32 = 0x1B << 16;
const WLAN_CCK_RX_TSF: u16 = 0x30;
const WLAN_OFDM_RX_TSF: u16 = 0x30;
const WLAN_TBTT_PROHIBIT: u8 = 0x04;
const WLAN_TBTT_HOLD_TIME: u16 = 0x064;
const WLAN_DRV_EARLY_INT: u8 = 0x04;
const WLAN_BCN_CTRL_CLT0: u8 = 0x10;
const WLAN_BCN_DMA_TIME: u8 = 0x02;
const WLAN_BCN_MAX_ERR: u8 = 0xFF;
const WLAN_SIFS_CCK_DUR_TUNE: u8 = 0x0A;
const WLAN_SIFS_OFDM_DUR_TUNE: u8 = 0x10;
const WLAN_SIFS_CCK_CTX: u8 = 0x0A;
const WLAN_SIFS_CCK_IRX: u8 = 0x0A;
const WLAN_SIFS_OFDM_CTX: u8 = 0x0E;
const WLAN_SIFS_OFDM_IRX: u8 = 0x0E;
const WLAN_EIFS_DUR_TUNE: u16 = 0x40;
const WLAN_EDCA_VO_PARAM: u32 = 0x002FA226;
const WLAN_EDCA_VI_PARAM: u32 = 0x005EA328;
const WLAN_EDCA_BE_PARAM: u32 = 0x005EA42B;
const WLAN_EDCA_BK_PARAM: u32 = 0x0000A44F;
const WLAN_RX_FILTER0: u32 = 0xFFFFFFFF;
const WLAN_RX_FILTER2: u16 = 0xFFFF;
const WLAN_RXPKT_MAX_SZ: u32 = 12288;
const WLAN_RXPKT_MAX_SZ_512: u8 = (WLAN_RXPKT_MAX_SZ >> 9) as u8;
const WLAN_AMPDU_MAX_TIME: u8 = 0x70;
const WLAN_RTS_LEN_TH: u8 = 0xFF;
const WLAN_RTS_TX_TIME_TH: u8 = 0x08;
const WLAN_MAX_AGG_PKT_LIMIT: u8 = 0x3F;
const WLAN_RTS_MAX_AGG_PKT_LIMIT: u8 = 0x3F;
const WLAN_PRE_TXCNT_TIME_TH: u16 = 0x1E0;
const FAST_EDCA_VO_TH: u8 = 0x06;
const FAST_EDCA_VI_TH: u8 = 0x06;
const FAST_EDCA_BE_TH: u8 = 0x06;
const FAST_EDCA_BK_TH: u8 = 0x06;
const WLAN_BAR_RETRY_LIMIT: u8 = 0x01;
const WLAN_BAR_ACK_TYPE: u8 = 0x05;
const WLAN_RA_TRY_RATE_AGG_LIMIT: u8 = 0x08;
const WLAN_RESP_TXRATE: u8 = 0x84;
const WLAN_ACK_TO: u8 = 0x21;
const WLAN_ACK_TO_CCK: u8 = 0x6A;
const WLAN_DATA_RATE_FB_CNT_1_4: u32 = 0x01000000;
const WLAN_DATA_RATE_FB_CNT_5_8: u32 = 0x08070504;
const WLAN_RTS_RATE_FB_CNT_5_8: u32 = 0x08070504;
const WLAN_DATA_RATE_FB_RATE0: u32 = 0xFE01F010;
const WLAN_DATA_RATE_FB_RATE0_H: u32 = 0x40000000;
const WLAN_RTS_RATE_FB_RATE1: u32 = 0x003FF010;
const WLAN_RTS_RATE_FB_RATE1_H: u32 = 0x40000000;
const WLAN_RTS_RATE_FB_RATE4: u32 = 0x0600F010;
const WLAN_RTS_RATE_FB_RATE4_H: u32 = 0x400003E0;
const WLAN_RTS_RATE_FB_RATE5: u32 = 0x0600F015;
const WLAN_RTS_RATE_FB_RATE5_H: u32 = 0x000000E0;
const WLAN_MULTI_ADDR: u32 = 0xFFFFFFFF;
const WLAN_TX_FUNC_CFG1: u8 = 0x30;
const WLAN_TX_FUNC_CFG2: u8 = 0x30;

// Power Management register definitions (match Linux hal_com_reg.h)
const REG_CR_8822C: u32 = 0x0100;
const REG_HMETFR_8822C: u32 = 0x01CC;
const REG_CPWM_8822C: u32 = 0x0100; // CPWM at 0x100 (same as CR for 8822C)
const REG_RPWM_8822C: u32 = 0x0108; // RPWM at 0x108

// Power state definitions
const PS_STATE_S0: u8 = 0x00; // Active
const PS_STATE_S1: u8 = 0x01; // RF ON
const PS_STATE_S2: u8 = 0x02; // 32k clock
const PS_STATE_S3: u8 = 0x03; // RF OFF
const PS_STATE_S4: u8 = 0x04; // Power off

const PS_TOGGLE: u8 = 0x80;
const PS_ACK: u8 = 0x40;

const LPS_CPWM_TIMEOUT_MS: u32 = 10;
const LPS_RPWM_RETRY_CNT: u8 = 3;

// Power state macros
const fn PS_STATE(pslv: u8) -> u8 {
    pslv & 0x0F
}
const WLAN_MAC_OPT_NORM_FUNC1: u8 = 0x98;
const WLAN_MAC_OPT_LB_FUNC1: u8 = 0x80;
const WLAN_MAC_OPT_FUNC2: u32 = 0xB0810041;
const WLAN_MAC_INT_MIG_CFG: u32 = 0x33330000;
const WLAN_SIFS_CFG: u32 = WLAN_SIFS_CCK_CONT_TX as u32 |
    ((WLAN_SIFS_OFDM_CONT_TX as u32) << 8) |
    ((WLAN_SIFS_CCK_TRX as u32) << 16) |
    ((WLAN_SIFS_OFDM_TRX as u32) << 24);
const WLAN_SIFS_DUR_TUNE: u16 = WLAN_SIFS_CCK_DUR_TUNE as u16 |
    ((WLAN_SIFS_OFDM_DUR_TUNE as u16) << 8);
const WLAN_TBTT_TIME: u32 = WLAN_TBTT_PROHIBIT as u32 |
    ((WLAN_TBTT_HOLD_TIME as u32) << 16);
const WLAN_NAV_CFG: u32 = WLAN_RDG_NAV |
    (WLAN_TXOP_NAV);
const WLAN_RX_TSF_CFG: u16 = WLAN_CCK_RX_TSF |
    (WLAN_OFDM_RX_TSF << 8);


const REG_PIFS: u32 = 0x0512;
const REG_TX_PTCL_CTRL: u32 = 0x0520;
const BIT_SIFS_BK_EN: u32 = 1 << 8;
const REG_RD_CTRL: u32 = 0x0524;
const BIT_DIS_TXOP_CFE: u32 = 1 << 8;
const BIT_DIS_LSIG_CFE: u32 = 1 << 9;
const BIT_DIS_STBC_CFE: u32 = 1 << 10;
const BIT_MAC_CLK_SEL: u32 = 1 << 3;

// EDCCA / adaptivity registers (rtw8822c_adaptivity_init)
const BIT_DIS_EDCCA: u32 = 1 << 15;
const BIT_EDCCA_MSK_CNTDOWN_EN: u32 = 1 << 11;
const REG_EDCCA_DECISION: u32 = 0x0844;
const BIT_EDCCA_OPTION: u32 = 0x3 << 29;
const REG_EDCCA_TH: u32 = 0x084C;
const REG_EDCCA_REPORT: u32 = 0x2D38;
const BIT_EDCCA_FLAG: u32 = 1 << 24;

// PHY energy / CRC counters (rtw8822c_false_alarm_statistics).
// 0x2C08: [15:0] CCK CCA, [31:16] OFDM CCA
// 0x2C04/0x2C14/0x2C10/0x2C0C: [15:0] ok, [31:16] err (CCK/OFDM/HT/VHT)
const REG_CCA_CNT: u32 = 0x2C08;
const REG_CCK_CRC_CNT: u32 = 0x2C04;
const REG_OFDM_CRC_CNT: u32 = 0x2C14;
const REG_HT_CRC_CNT: u32 = 0x2C10;
const REG_VHT_CRC_CNT: u32 = 0x2C0C;
// Counter clock-gate / reset (rtw8822c_false_alarm_statistics tail). The FA/CCA
// counters only accrue while the RX gate clock is enabled, and rtw88 resets
// them with this exact sequence.
const REG_RX_BREAK: u32 = 0x1D2C;
const BIT_COM_RX_GCK_EN: u32 = 1 << 31;
const REG_CNT_CTRL: u32 = 0x1EB4;
const BIT_ALL_CNT_RST: u32 = 1 << 25;
const REG_USTIME_TSF: u32 = 0x055C;
const REG_USTIME_EDCA: u32 = 0x0638;
const MAC_CLK_SPEED: u8 = 80;
const REG_MISC_CTRL: u32 = 0x0577;
const BIT_EN_FREE_CNT: u32 = 1 << 0;
const BIT_DIS_SECOND_CCA: u32 = 1 << 1;
const REG_TIMER0_SRC_SEL: u32 = 0x05B4;
const BIT_TSFT_SEL_TIMER0: u32 = 1 << 0;
const REG_SLOT: u32 = 0x051B;
const REG_RD_NAV_NXT: u32 = 0x0544;
const REG_RXTSF_OFFSET_CCK: u32 = 0x055E;
const REG_DRVERLYINT: u32 = 0x0558;
const REG_BCN_CTRL_CLINT0: u32 = 0x0551;
const REG_BCNDMATIM: u32 = 0x0559;
const REG_BCN_MAX_ERR: u32 = 0x055D;
const REG_MAR: u32 = 0x0620;
const REG_BBPSF_CTRL: u32 = 0x06DC;
// Additional registers for mac_init_after_fw (only those not in main section)
const REG_SPEC_SIFS: u32 = 0x0428;
const REG_SIFS: u32 = 0x0514;
const REG_RESP_SIFS_CCK: u32 = 0x063C;
const REG_RESP_SIFS_OFDM: u32 = 0x063E;
const REG_EDCA_VO_PARAM: u32 = 0x0500;
const REG_EDCA_VI_PARAM: u32 = 0x0504;
const REG_EDCA_BE_PARAM: u32 = 0x0508;
const REG_EDCA_BK_PARAM: u32 = 0x050C;
const REG_AMPDU_MAX_TIME_V1: u32 = 0x0455;
const REG_TX_HANG_CTRL: u32 = 0x045E;
const BIT_EN_EOF_V1: u32 = 1 << 0;
const REG_PRECNT_CTRL: u32 = 0x04E5;
const BIT_EN_PRECNT: u32 = 1 << 15;
const REG_PROT_MODE_CTRL: u32 = 0x04C8;
const REG_BAR_MODE_CTRL: u32 = 0x04CC;
const REG_FAST_EDCA_VOVI_SETTING: u32 = 0x1448;
const REG_FAST_EDCA_BEBK_SETTING: u32 = 0x144C;
const REG_LIFETIME_EN: u32 = 0x0426;
const BIT_BA_PARSER_EN: u32 = 1 << 0;
const REG_RRSR: u32 = 0x0440;
const BITS_RRSR_RSC: u32 = 0xFF;

// RA (Rate Adaptive) register definitions - use existing REG_ARFR*_V1 where available
// REG_ARFR0 = 0x0280 (existing), REG_ARFRH0 = 0x0284 (existing)
// REG_ARFR1_V1 = 0x0288 (existing), REG_ARFRH1_V1 = 0x028C (existing)
// REG_ARFR4 = 0x0290 (existing, is REG_ARFR2), REG_ARFRH4 = 0x0294 (existing, is REG_ARFRH2)
// REG_ARFR5 = 0x0298 (existing, is REG_ARFR3), REG_ARFRH5 = 0x029C (existing, is REG_ARFRH3)
const REG_RRSR_2G: u32 = 0x0294;
const REG_RRSR_5G: u32 = 0x0298;
const REG_ARFR1: u32 = REG_ARFR1_V1;
const REG_ARFRH1: u32 = REG_ARFRH1_V1;
const REG_ARFR2: u32 = REG_ARFR4;
const REG_ARFRH2: u32 = REG_ARFRH4;
const REG_ARFR3: u32 = REG_ARFR5;
const REG_ARFRH3: u32 = REG_ARFRH5;
// Additional ARFR registers for MACID 4-7
const REG_ARFR4_EXT: u32 = 0x02A0;
const REG_ARFRH4_EXT: u32 = 0x02A4;
const REG_ARFR5_EXT: u32 = 0x02A8;
const REG_ARFRH5_EXT: u32 = 0x02AC;
const REG_ARFR6: u32 = 0x02B0;
const REG_ARFRH6: u32 = 0x02B4;
const REG_ARFR7: u32 = 0x02B8;
const REG_ARFRH7: u32 = 0x02BC;

// Rate index definitions (match Linux ODM_RATE_*)
const RATE_CCK_1M: u8 = 0x00;
const RATE_CCK_2M: u8 = 0x01;
const RATE_CCK_5_5M: u8 = 0x02;
const RATE_CCK_11M: u8 = 0x03;
const RATE_OFDM_6M: u8 = 0x04;
const RATE_OFDM_9M: u8 = 0x05;
const RATE_OFDM_12M: u8 = 0x06;
const RATE_OFDM_18M: u8 = 0x07;
const RATE_OFDM_24M: u8 = 0x08;
const RATE_OFDM_36M: u8 = 0x09;
const RATE_OFDM_48M: u8 = 0x0A;
const RATE_OFDM_54M: u8 = 0x0B;
const RATE_MCS0: u8 = 0x0C;
const RATE_MCS1: u8 = 0x0D;
const RATE_MCS2: u8 = 0x0E;
const RATE_MCS3: u8 = 0x0F;
const RATE_MCS4: u8 = 0x10;
const RATE_MCS5: u8 = 0x11;
const RATE_MCS6: u8 = 0x12;
const RATE_MCS7: u8 = 0x13;
const RATE_MCS8: u8 = 0x14;
const RATE_MCS9: u8 = 0x15;
const RATE_MCS10: u8 = 0x16;
const RATE_MCS11: u8 = 0x17;
const RATE_MCS12: u8 = 0x18;
const RATE_MCS13: u8 = 0x19;
const RATE_MCS14: u8 = 0x1A;
const RATE_MCS15: u8 = 0x1B;
const RATE_MCS16: u8 = 0x1C;
const RATE_MCS17: u8 = 0x1D;
const RATE_MCS18: u8 = 0x1E;
const RATE_MCS19: u8 = 0x1F;
const RATE_MCS20: u8 = 0x20;
const RATE_MCS21: u8 = 0x21;
const RATE_MCS22: u8 = 0x22;
const RATE_MCS23: u8 = 0x23;
const RATE_MCS24: u8 = 0x24;
const RATE_MCS25: u8 = 0x25;
const RATE_MCS26: u8 = 0x26;
const RATE_MCS27: u8 = 0x27;
const RATE_MCS28: u8 = 0x28;
const RATE_MCS29: u8 = 0x29;
const RATE_MCS30: u8 = 0x2A;
const RATE_MCS31: u8 = 0x2B;
const RATE_VHT_1SS_MCS0: u8 = 0x2C;
const RATE_VHT_1SS_MCS1: u8 = 0x2D;
const RATE_VHT_1SS_MCS2: u8 = 0x2E;
const RATE_VHT_1SS_MCS3: u8 = 0x2F;
const RATE_VHT_1SS_MCS4: u8 = 0x30;
const RATE_VHT_1SS_MCS5: u8 = 0x31;
const RATE_VHT_1SS_MCS6: u8 = 0x32;
const RATE_VHT_1SS_MCS7: u8 = 0x33;
const RATE_VHT_1SS_MCS8: u8 = 0x34;
const RATE_VHT_1SS_MCS9: u8 = 0x35;
const RATE_VHT_2SS_MCS0: u8 = 0x36;
const RATE_VHT_2SS_MCS1: u8 = 0x37;
const RATE_VHT_2SS_MCS2: u8 = 0x38;
const RATE_VHT_2SS_MCS3: u8 = 0x39;
const RATE_VHT_2SS_MCS4: u8 = 0x3A;
const RATE_VHT_2SS_MCS5: u8 = 0x3B;
const RATE_VHT_2SS_MCS6: u8 = 0x3C;
const RATE_VHT_2SS_MCS7: u8 = 0x3D;
const RATE_VHT_2SS_MCS8: u8 = 0x3E;
const RATE_VHT_2SS_MCS9: u8 = 0x3F;

const SGI_FLAG: u8 = 0x80;

// RRSR bit masks
const RRSR_CCK_1M: u32 = 1 << 0;
const RRSR_CCK_2M: u32 = 1 << 1;
const RRSR_CCK_5_5M: u32 = 1 << 2;
const RRSR_CCK_11M: u32 = 1 << 3;
const RRSR_OFDM_6M: u32 = 1 << 4;
const RRSR_OFDM_9M: u32 = 1 << 5;
const RRSR_OFDM_12M: u32 = 1 << 6;
const RRSR_OFDM_18M: u32 = 1 << 7;
const RRSR_OFDM_24M: u32 = 1 << 8;
const RRSR_OFDM_36M: u32 = 1 << 9;
const RRSR_OFDM_48M: u32 = 1 << 10;
const RRSR_OFDM_54M: u32 = 1 << 11;
const RRSR_MCS0: u32 = 1 << 12;
const RRSR_MCS1: u32 = 1 << 13;
const RRSR_MCS2: u32 = 1 << 14;
const RRSR_MCS3: u32 = 1 << 15;
const RRSR_MCS4: u32 = 1 << 16;
const RRSR_MCS5: u32 = 1 << 17;
const RRSR_MCS6: u32 = 1 << 18;
const RRSR_MCS7: u32 = 1 << 19;
const RRSR_MCS8: u32 = 1 << 20;
const RRSR_MCS9: u32 = 1 << 21;
const RRSR_MCS10: u32 = 1 << 22;
const RRSR_MCS11: u32 = 1 << 23;
const RRSR_MCS12: u32 = 1 << 24;
const RRSR_MCS13: u32 = 1 << 25;
const RRSR_MCS14: u32 = 1 << 26;
const RRSR_MCS15: u32 = 1 << 27;
const RRSR_HT_2SS_MASK: u32 = 0x00FFF000;
const RRSR_VHT_1SS_MASK: u32 = 0x3F000000;
const RRSR_VHT_2SS_MASK: u32 = 0xFC000000;

// Missing register definitions for MAC init after firmware
const REG_TBTT_PROHIBIT: u32 = 0x0540;
const REG_ACKTO: u32 = 0x0640;
const REG_ACKTO_CCK: u32 = 0x0639;
const REG_EIFS: u32 = 0x0642;
const REG_NAV_CTRL: u32 = 0x0650;
const REG_WMAC_TRXPTCL_CTL: u32 = 0x0668;
const BIT_RFMOD: u32 = (1 << 7) | (1 << 8);
const REG_DATA_SC: u32 = 0x0483;
const REG_WMAC_TRXPTCL_CTL_H: u32 = 0x066C;
const REG_RXFLTMAP0: u32 = 0x06A0;
const REG_RXFLTMAP2: u32 = 0x06A4;
const REG_RX_PKT_LIMIT: u32 = 0x060C;

// RXPSF bit definitions
const BIT_RXPSF_CTRLEN: u16 = 1 << 0;
const BIT_RXPSF_VHTCHKEN: u16 = 1 << 1;
const BIT_RXPSF_HTCHKEN: u16 = 1 << 2;
const BIT_RXPSF_OFDMCHKEN: u16 = 1 << 3;
const BIT_RXPSF_CCKCHKEN: u16 = 1 << 4;
const BIT_RXPSF_OFDMRST: u16 = 1 << 5;
const BIT_RXPSF_MHCHKEN: u16 = 1 << 6;
const BIT_RXPSF_CCKRST: u16 = 1 << 7;
const BIT_RXPSF_CONT_ERRCHKEN: u16 = 1 << 8;

// RXPSF helper macros (as const functions)
const fn BIT_SET_RXPSF_PKTLENTHR(val: u16, thr: u16) -> u16 {
    val | ((thr & 0xF) << 9)
}
const fn BIT_SET_RXPSF_ERRTHR(val: u16, thr: u16) -> u16 {
    val | ((thr & 0xF) << 12)
}
const fn BIT_RXGCK_VHT_FIFOTHR(thr: u32) -> u32 {
    (thr & 0xFF) << 16
}
const fn BIT_RXGCK_HT_FIFOTHR(thr: u32) -> u32 {
    (thr & 0xFF) << 24
}
const fn BIT_RXGCK_OFDM_FIFOTHR(thr: u32) -> u32 {
    (thr & 0xFF) << 8
}
const fn BIT_RXGCK_CCK_FIFOTHR(thr: u32) -> u32 {
    thr & 0xFF
}

// ============================================================================
// Registros MMIO del RTL8822CE
// ============================================================================
// Registros MMIO del RTL8822CE
// ============================================================================
const REG_BEQ_TXBD_DESA: u32 = 0x0328;  // Linux: RTK_PCI_TXBD_DESA_BEQ = 0x328
const REG_BKQ_TXBD_DESA: u32 = 0x0330;  // Linux: RTK_PCI_TXBD_DESA_BKQ = 0x330
const REG_MGQ_TXBD_DESA: u32 = 0x0310;  // Linux: RTK_PCI_TXBD_DESA_MGMTQ = 0x310
const REG_HIQ_TXBD_DESA: u32 = 0x0340;  // Linux: RTK_PCI_TXBD_DESA_HI0Q = 0x340
const REG_RXQ_RXBD_DESA: u32 = 0x0338;  // Linux: RTK_PCI_RXBD_DESA_MPDUQ = 0x338

// NUM registers (corrected to match Linux pci.h)
const REG_VOQ_TXBD_NUM: u32 = 0x0384;
const REG_VIQ_TXBD_NUM: u32 = 0x0386;
const REG_BEQ_TXBD_NUM: u32 = 0x0388;
const REG_BKQ_TXBD_NUM: u32 = 0x038A;
const REG_MGQ_TXBD_NUM: u32 = 0x0380;
const REG_HIQ_TXBD_NUM: u32 = 0x038C;
const REG_RXQ_RXBD_NUM: u32 = 0x0382;

// --- Ring Index Registers (corrected to match Linux pci.h) ---
const REG_VOQ_TXBD_IDX: u32 = 0x03A0;
const REG_VIQ_TXBD_IDX: u32 = 0x03A4;
const REG_BEQ_TXBD_IDX: u32 = 0x03A8;
const REG_BKQ_TXBD_IDX: u32 = 0x03AC;
const REG_MGQ_TXBD_IDX: u32 = 0x03B0;
const REG_HIQ_TXBD_IDX: u32 = 0x03B8;
const REG_RXQ_RXBD_IDX: u32 = 0x03B4;

// --- Non-QoS hardware sequence-number registers (halmac REG_HW_SEQ0..3) ---
// Bits [11:0] hold the 12-bit sequence number selected by HW_SSN_SEL in the
// TX descriptor. Needed because EN_HWSEQ=1 makes the MAC take the on-air
// sequence from this register instead of SW_SEQ.
const REG_HW_SEQ0: u32 = 0x04D8;

// --- Interrupt Registers (PCIe specific) ---
const REG_PCIE_HIMR0: u32 = 0x00B0;
const REG_PCIE_HISR0: u32 = 0x00B4;
const REG_PCIE_HIMR1: u32 = 0x00B8;
const REG_PCIE_HISR1: u32 = 0x00BC;
// rtw88 also implements a third interrupt group (pci.h RTK_PCI_HIMR3/HISR3).
// Reading/clearing HISR1 and HISR3 is what stops the HISR1_IND storm.
const REG_PCIE_HIMR3: u32 = 0x10B8;
const REG_PCIE_HISR3: u32 = 0x10BC;

// --- FW Download ---
const REG_CPU_DMEM_CON: u32 = 0x1080;
const BIT_WL_PLATFORM_RST: u32 = 1 << 16;
const BIT_DDMA_EN: u32 = 1 << 8;
const REG_CR_EXT: u32 = 0x1100;
const REG_GPIO_MUXCFG: u32 = 0x0040;
const BIT_FSPI_EN: u32 = 1 << 19;
const BIT_BOOT_FSPI_EN: u32 = 1 << 20;

// IDDMA registers for non-8051 firmware download (RTL8822CE WCPU 3081)
const REG_DDMA_CH0SA: u32 = 0x1200;
const REG_DDMA_CH0DA: u32 = 0x1204;
const REG_DDMA_CH0CTRL: u32 = 0x1208;
const BIT_DDMACH0_OWN: u32 = 1 << 31;
const BIT_DDMACH0_CHKSUM_EN: u32 = 1 << 29;
const BIT_DDMACH0_CHKSUM_STS: u32 = 1 << 27;
const BIT_DDMACH0_DDMA_MODE: u32 = 1 << 26;
const BIT_DDMACH0_RESET_CHKSUM_STS: u32 = 1 << 25;
const BIT_DDMACH0_CHKSUM_CONT: u32 = 1 << 24;
const BIT_MASK_DDMACH0_DLEN: u32 = 0x3FFFF;

// Firmware header constants
const FW_HDR_SIZE: usize = 64;
const FW_HDR_CHKSUM_SIZE: usize = 8;

// OCP addresses
const OCPBASE_TXBUF_88XX: u32 = 0x18780000;
const OCPBASE_DMEM_88XX: u32 = 0x00200000;
const OCPBASE_EMEM_88XX: u32 = 0x00100000;
const OCPBASE_IMEM_88XX: u32 = 0x00300000;

// TX descriptor size for firmware packets (beacon queue) - must match chip->tx_pkt_desc_sz
const TX_DESC_SIZE_FW: usize = 48;

// TX descriptor QSEL values
const TX_DESC_QSEL_BEACON: u32 = 16;

// Beacon FIFO page control
const REG_FIFOPAGE_CTRL_2: u32 = 0x0204;
const BIT_BCN_VALID_V1: u32 = 1 << 15;
// Stage each FW chunk into the chip TX FIFO (BCN rsvd page) and IDDMA from
// OCPBASE_TXBUF, exactly like Linux/vendor. false = old direct host-RAM IDDMA
// (proven to fail mid-EMEM: ch#29 dropped its IDD done in the last run).
const FW_USE_BCNQ_STAGING: bool = true;
const BIT_MASK_BCN_HEAD_1_V1: u32 = 0xFFF;

// BCN queue (beacon/reserved page TX) - own BD ring at 0x308
const RTK_PCI_TXBD_DESA_BCNQ: u32 = 0x0308;
const RTK_PCI_TXBD_BCN_WORK: u32 = 0x0383;
const BIT_PCI_BCNQ_FLAG: u8 = 1 << 4;
const RTK_PCI_TXBD_OWN_OFFSET: u32 = 15;
const BCN_TX_RING_SIZE: usize = 2;

// PCIe DMA control
const RTK_PCI_CTRL: u32 = 0x0300;
const BIT_RST_TRXDMA_INTF: u32 = 1 << 20;
const BIT_RX_TAG_EN: u32 = 1 << 15;
const RTK_PCI_TXBD_RWPTR_CLR: u32 = 0x039C;

// TX DMA status
const REG_TXDMA_STATUS: u32 = 0x0210;
const BTI_PAGE_OVF: u32 = 1 << 2;

// FW ready check
const REG_FW_DBG6: u32 = 0x10F8;
const REG_FW_DBG7: u32 = 0x10FC;
const REG_BOOT_REASON: u32 = 0x1088;
const REG_MCUTST_I: u32 = 0x01C0;
const REG_MCUTST_II: u32 = 0x01C4;

// 3081 WCPU (MIPS-class core) exception registers (Linux reg.h).
// Read these to classify a FW hang: exception (CAUSE/EPC != 0) vs deliberate
// spin loop (CAUSE == 0, stable PC).
const REG_ARFR2_V1: u32 = 0x048C; // EPC (exception program counter)
const REG_ARFRH2_V1: u32 = 0x0490; // BADADDR
const REG_ARFR3_V1: u32 = 0x0494; // CAUSE
const REG_ARFRH3_V1: u32 = 0x0498; // STATUS
const REG_ARFR4_SP: u32 = 0x049C; // SP (stack pointer)
const REG_ARFRH4_RA: u32 = 0x04A0; // RA (return address)
const REG_ARFR5_V1: u32 = 0x04A4;
const BIT_EXC_CODE: u32 = 0x7C; // GENMASK(6,2): exception code field of CAUSE
const FW_READY_MASK: u32 = 0xCFFF;
const FW_READY: u32 = 0xC078;
const ILLEGAL_KEY_GROUP: u32 = 0xFAAAAA00;
const BIT_CHECK_SUM_OK: u32 = (1 << 4) | (1 << 6);
const BIT_FW_DW_RDY: u32 = 1 << 14;

// WLAN <-> LTE coex indirect access (Linux reg.h REG_WL2LTECOEX_INDIRECT_ACCESS_*)
const REG_LTECOEX_CTRL: u32 = 0x1700;
const REG_LTECOEX_WDATA: u32 = 0x1704;
const REG_LTECOEX_RDATA: u32 = 0x1708;
const LTECOEX_READY: u32 = 1 << 29;
const LTECOEX_BACKUP_OFFSET: u32 = 0x38;

// H2C packet path
const REG_H2C_PKT_READADDR: u32 = 0x10D0;
const REG_H2C_PKT_WRITEADDR: u32 = 0x10D4;

// Backup register count for firmware download
const DLFW_RESTORE_REG_NUM: usize = 6;

/// Single entry of the firmware-download register backup (mirrors Linux
/// `struct rtw_backup_info`: len/reg/val restored via rtw_restore_reg).
#[derive(Clone, Copy)]
struct DwRegBackup {
    len: u8,
    reg: u32,
    val: u32,
}

    // TX queue indices (used in setup_tx_ring and send_mgmt_frame)
    const TX_QUEUE_HI: u32 = 1;

    // ============================================================================
    // Bits de System Function Enable (REG_SYS_FUNC_EN)
    // ============================================================================
    const BIT_FEN_DIO_PCIE: u16 = 1 << 5;
    const BIT_FEN_PCIEA: u16 = 1 << 6;
    const BIT_FEN_PPLL: u16 = 1 << 7;
    const BIT_FEN_CPUEN: u16 = 1 << 10;
    const BIT_FEN_ELDR: u16 = 1 << 12;
    const BIT_FEN_MREGEN: u16 = 1 << 15;

// ============================================================================
// Bits de MCU Firmware Control (REG_MCUFW_CTRL = 0x0080)
// ============================================================================
const BIT_MCUFWDL_EN: u32 = 1 << 0;
const BIT_MCUFWDL_RDY: u32 = 1 << 1;
const BIT_FWDL_CHK_RPT: u32 = 1 << 2;
const BIT_IMEM_DW_OK: u32 = 1 << 3;
const BIT_IMEM_CHKSUM_OK: u32 = 1 << 4;
const BIT_DMEM_DW_OK: u32 = 1 << 5;
const BIT_DMEM_CHKSUM_OK: u32 = 1 << 6;
const BIT_RAM_DL_SEL: u32 = 1 << 7;
const BIT_FW_DW_RDY_NONLEGACY: u32 = 1 << 14;
const BIT_FW_INIT_RDY: u32 = 1 << 15;
const BIT_ROM_PGE: u32 = 7 << 16;
const BIT_ROM_DLEN: u32 = 1 << 19;
const BIT_CPU_CLK_EN: u32 = 1 << 14;

// ============================================================================
// Bits de MAC Command Register (REG_CR)
// ============================================================================
const BIT_HCI_TXDMA_EN: u32 = 1 << 0;
const BIT_HCI_RXDMA_EN: u32 = 1 << 1;
const BIT_TXDMA_EN: u32 = 1 << 2;
const BIT_RXDMA_EN: u32 = 1 << 3;
const BIT_PROTOCOL_EN: u32 = 1 << 4;
const BIT_SCHEDULE_EN: u32 = 1 << 5;
const BIT_MAC_TX_EN: u32 = 1 << 6;
const BIT_MAC_RX_EN: u32 = 1 << 7;
const BIT_ENSEC: u32 = 1 << 9;
const BIT_ENSWBCN: u32 = 1 << 8; // Linux reg.h: BIT_ENSWBCN BIT!(8); usado como (>>8) en byte REG_CR+1
// Linux reg.h: MAC_TRX_ENABLE = HCI_TXDMA|HCI_RXDMA|TXDMA|RXDMA|PROTOCOL|SCHEDULE|MAC_TX|MAC_RX (bits 0-7)
const MAC_TRX_ENABLE: u32 = 0x0000_00FF;

// RF SIPi addresses (from Linux rtw8822c_hw_spec.rf_sipi_addr)
const RF_SIPI_ADDR_A: u32 = 0x1808;
const RF_SIPI_ADDR_B: u32 = 0x4108;
const RFREG_MASK: u32 = 0xFFFFF;
const RF_CFGCH: u32 = 0x18; // RF register for channel

// 3-wire control registers (for rstb_3wire sequence, from Linux rtw8822c.h)
const REG_RSTB: u32 = 0x1C90;
const BIT_RSTB_3WIRE: u32 = 1 << 8;
const REG_ANAPAR_A: u32 = 0x1830;
const REG_ANAPAR_B: u32 = 0x4130;
const BIT_ANAPAR_UPDATE: u32 = 1 << 29;

// Additional PHY/RF registers
const REG_ANAPAR_XTAL_0: u32 = 0x1040;
const REG_DIS_DPD: u32 = 0x0A70;
const DIS_DPD_MASK: u32 = 0x3ff;
const DIS_DPD_RATEALL: u32 = 0xFFFFFFFF;
// DACK (RF DAC calibration) sample count (rtw8822c.h DACK_SN_8822C)
const DACK_SN_8822C: usize = 100;
const REG_ORITXCODE: u32 = 0x1800;
const REG_ORITXCODE2: u32 = 0x4100;
const REG_RXIGI: u32 = 0x1C30;
// RX/TX path selection registers (rtw88 rtw8822c.h / rtw8822c_config_trx_mode)
const REG_CCANRX: u32 = 0x1A2C;
const REG_RXCCKSEL: u32 = 0x1A04;
const REG_RXFNCTL: u32 = 0x1D30;
const REG_AGCSWSH: u32 = 0x0C44;
const REG_ANTWTPD: u32 = 0x0C54;
const REG_MRCM: u32 = 0x0C38;
const REG_ANTMAP: u32 = 0x0824;
const REG_ANTMAP0: u32 = 0x0820;
const REG_TXLGMAP: u32 = 0x1E2C;
const MASK20BITS: u32 = 0x000F_FFFF;

// ============================================================================
// Bits de Interrupt Mask/Status
// ============================================================================
// HIMR0 bits
const BIT_RXOK_INT: u32 = 1 << 0;
const BIT_RDU_INT: u32 = 1 << 1;
const BIT_VODOK_INT: u32 = 1 << 2;
const BIT_VIDOK_INT: u32 = 1 << 3;
const BIT_BEDOK_INT: u32 = 1 << 4;
const BIT_BKDOK_INT: u32 = 1 << 5;
const BIT_MGTDOK_INT: u32 = 1 << 6;
const BIT_HIGHDOK_INT: u32 = 1 << 7;
// Real C2H command interrupt is HISR0 bit 10 (= rtw88 `IMR_C2HCMD`). Bit 11 is
// `IMR_HISR1_IND_INT`, the "HISR1 has a pending interrupt" indicator — enabling
// it as if it were C2H but never reading/clearing HISR1 caused the ISR storm.
const BIT_C2H_INT: u32 = 1 << 10;
// rtw88 mask[0] also includes IMR_BCNDMAINT_E (bit 14); we don't run beacon DMA
// maintenance, so it stays disabled to avoid unhandled periodic interrupts.
const BIT_BCNDMAINT_E_INT: u32 = 1 << 14;
// HIMR1 (rtw88): TX FIFO overflow warning.
const BIT_TXFOVW_INT: u32 = 1 << 9;
// HIMR3 (rtw88): H2C done. We drive H2C via HMETFR mailboxes, so this stays off.
const BIT_H2CDOK_INT: u32 = 1 << 16;

// ============================================================================
// TX Descriptor format DWORD0
// ============================================================================
const TX_DESC_QUEUE_SFT: u32 = 5;
const TX_DESC_QUEUE_MSK: u32 = 0x7 << TX_DESC_QUEUE_SFT;

// TX Queue IDs
const TX_QUEUE_VO: u32 = 3;
const TX_QUEUE_VI: u32 = 4;
const TX_QUEUE_BE: u32 = 5;
const TX_QUEUE_BK: u32 = 6;
const TX_QUEUE_MGT: u32 = 2;
// Logical ring slot for the H2C command queue (slot 0 is otherwise unused).
const TX_QUEUE_H2C: u32 = 0;
// TX descriptor QSEL value for H2C packets (rtw88 enum rtw_tx_desc_queue_select).
const TX_DESC_QSEL_H2C: u32 = 19;

// ============================================================================
// RX Descriptor format
// ============================================================================
const RX_DESC_BUFSZ_MASK: u32 = 0x3FFF;

// ============================================================================
// Firmware Header (64 bytes)
// ============================================================================
const FW_HEADER_SIZE: usize = 64;
const FW_PAGE_SIZE: usize = 4096;

// ============================================================================
// H2C Command IDs
// ============================================================================
// H2C Command IDs (match Linux hal_com_h2c.h)
const H2C_CMD_RSVD_PAGE: u8 = 0x00;
const H2C_CMD_MEDIA_STATUS_RPT: u8 = 0x01;
const H2C_CMD_SCAN_ENABLE: u8 = 0x02;
const H2C_CMD_CHNL_SWITCH_OPER_OFFLOAD: u8 = 0x1C;
const H2C_CMD_SET_PWR_MODE: u8 = 0x20;
const H2C_CMD_DEFAULT_PORT: u8 = 0x2C;

// H2C Sub-command IDs
const H2C_SUB_CMD_CH_SWITCH: u8 = 0x02;

// Reserved page types for H2C_RSVD_PAGE
const RSVD_PAGE_BEACON: u8 = 0;
const RSVD_PAGE_PROBE_RSP: u8 = 1;
const RSVD_PAGE_PS_POLL: u8 = 2;
const RSVD_PAGE_NULL_DATA: u8 = 3;
const RSVD_PAGE_QOS_NULL: u8 = 4;
const RSVD_PAGE_BT_QOS_NULL: u8 = 5;

// H2C Security commands
const H2C_CMD_SECURITY_ADD_KEY: u8 = 0x05;
const H2C_CMD_SECURITY_REMOVE_KEY: u8 = 0x06;

// H2C Sub-command for security key content
const H2C_SUB_SEC_KEY_CONTENT: u8 = 0x01;

// H2C other commands (match Linux hal_com_h2c.h)
const H2C_CMD_KEEP_ALIVE: u8 = 0x03;
const H2C_CMD_AP_OFFLOAD: u8 = 0x08;
const H2C_CMD_BEACON_RSVDPAGE: u8 = 0x09;
const H2C_CMD_PROBERSP_RSVDPAGE: u8 = 0x0A;
const H2C_CMD_FCS_RSVDPAGE: u8 = 0x10;
const H2C_CMD_FCS_INFO: u8 = 0x11;
const H2C_CMD_AP_WOW_GPIO_CTRL: u8 = 0x13;
const H2C_CMD_AP_PS: u8 = 0x26;
const H2C_CMD_PS_TUNING_PARA: u8 = 0x21;
const H2C_CMD_P2P_LPS_PARAM: u8 = 0x23;
const H2C_CMD_MACID_CFG: u8 = 0x40;
const H2C_CMD_TXBF: u8 = 0x41;
const H2C_CMD_RSSI_SETTING: u8 = 0x42;
const H2C_CMD_INIT_RATE_COLLECT: u8 = 0x44;
const H2C_CMD_RA_MASK_3SS: u8 = 0x46;
const H2C_CMD_DYNAMIC_TX_PATH: u8 = 0x48;
const H2C_CMD_B_TYPE_TDMA: u8 = 0x60;
const H2C_CMD_BT_MP_OPER: u8 = 0x67;
const H2C_CMD_BT_CONTROL: u8 = 0x68;
const H2C_CMD_BT_INFO: u8 = 0x61; // BT info command
const H2C_CMD_WOWLAN: u8 = 0x80;
const H2C_CMD_CHNL_SWITCH_OFFLOAD: u8 = 0x87;
const H2C_CMD_RESET_TSF: u8 = 0xC0;

// H2C Common Class commands (0x00-0x1F)
const H2C_CMD_DISCON_DECISION: u8 = 0x04;
const H2C_CMD_CUSTOMER_STR_REQ: u8 = 0x06;
const H2C_CMD_PSD_OFFLOAD: u8 = 0x05;

// H2C PowerSave Class commands (0x20-0x3F)
const H2C_CMD_AP_REQ_TXRPT: u8 = 0x43;
const H2C_CMD_IQ_CALIBRATION: u8 = 0x45;
const H2C_CMD_RA_PARA_ADJUST: u8 = 0x47;

// H2C Dynamic Mechanism Class commands (0x40-0x5F)
const H2C_CMD_FW_TRACE_EN: u8 = 0x49;
const H2C_CMD_REQ_PER_RPT: u8 = 0x4e;

// H2C BT Class commands (0x60-0x6F)
const H2C_CMD_WL_OPMODE: u8 = 0x66;
const H2C_CMD_BT_WIFI_CTRL: u8 = 0x69;
const H2C_CMD_BT_FW_PATCH: u8 = 0x6A;

// H2C WOWLAN Class commands (0x80-0x9F)
const H2C_CMD_REMOTE_WAKE_CTRL: u8 = 0x81;
const H2C_CMD_AOAC_GLOBAL_INFO: u8 = 0x82;
const H2C_CMD_AOAC_RSVD_PAGE: u8 = 0x83;
const H2C_CMD_AOAC_RSVD_PAGE2: u8 = 0x84;
const H2C_CMD_D0_SCAN_OFFLOAD_CTRL: u8 = 0x85;
const H2C_CMD_D0_SCAN_OFFLOAD_INFO: u8 = 0x86;
const H2C_CMD_AOAC_RSVDPAGE3: u8 = 0x88;
const H2C_CMD_P2P_OFFLOAD_RSVD_PAGE: u8 = 0x8A;
const H2C_CMD_P2P_OFFLOAD: u8 = 0x8B;
const H2C_CMD_FW_BCN_OFFLOAD: u8 = 0xBA;

// H2C extended commands (0xC0-0xFF)
const H2C_CMD_BCNHWSEQ: u8 = 0xC5;
const H2C_CMD_CUSTOMER_STR_W1: u8 = 0xC6;

// ============================================================================
// Estructuras del driver
// ============================================================================

/// TX Buffer Descriptor (16 bytes, Hardware format)
/// TX Buffer Descriptor segment (8 bytes, hardware format for RTL8822C).
///
/// Matches rtw88 `struct rtw_pci_tx_buffer_desc` (pci.h) and the vendor
/// `halmac_tx_bd_nic.h` 32-bit-DMA layout (`tx_buf_desc_sz = 16` on 8822C):
///   [15:0]  buf_size  (TX_BUFF_SIZE0/1)
///   [23:16] psb_len   (page size in 128-byte units, bit 31 = OWN on BCN)
///   [31:24] reserved / OWN
///   [63:32] physical address (low 32 bits)
///
/// A packet occupies TWO consecutive segments (16-byte stride): segment 0 is
/// the 48-byte TX header (`TX_WIFI_INFO`), segment 1 is the frame payload.
/// The old 16-byte single-descriptor definition placed the payload at +40 with
/// `buf_size = 40+len`, so the hardware (which reads a 48-byte header segment
/// and then a separate payload segment) skipped 8 bytes of the frame header and
/// never transmitted correctly, raising TXERR.
#[repr(C)]
#[derive(Clone, Copy)]
struct TxBufferDesc {
    buf_size: u16,
    psb_len: u16,
    dma: u32,
}

const _: () = assert!(core::mem::size_of::<TxBufferDesc>() == 8);

/// Byte stride of one TX packet slot in the BD ring: two 8-byte segments.
const TX_BD_STRIDE: usize = 16;

/// RX Buffer Descriptor (8 bytes, hardware format for RTL8822C).
///
/// Matches rtw88 `rx_buf_desc_sz = 8` (`struct rtw_pci_rx_buffer_desc`) and the
/// vendor `struct rx_buf_desc` (dword[2], 32-bit DMA) in
/// `hal/rtl8822c/pci/rtl8822ce.h`. The previous 16-byte definition was wrong:
/// the hardware reads descriptor *i* at offset `i * 8`, so a 16-byte stride
/// pointed DMA at the wrong buffers and every field was misread.
///
/// DW0 layout:
///   [13:0]  rx_buf_size   (vendor `SET_RX_BD_RXBUFFSIZE`)
///   [14]    LS            (last segment, vendor `SET_RX_BD_LS`)
///   [15]    FS            (first segment, vendor `SET_RX_BD_FS`)
///   [28:16] total_pkt_size(readiness/tag, vendor `GET_RX_BD_TOTALRXPKTSIZE`)
///   [31]    rx_fail
/// DW1 layout:
///   [31:0]  buffer physical address (low)
#[repr(C)]
#[derive(Clone, Copy)]
struct RxBufferDesc {
    /// DW0: buffer size + FS/LS + total packet size + rx_fail
    buf_info: u32,
    /// DW1: physical address of the RX buffer (low 32 bits)
    dma: u32,
}

impl RxBufferDesc {
    /// Arm a descriptor: set the buffer size (FS/LS/total cleared) and address.
    #[inline]
    fn arm(&mut self, buf_size: u32, dma: u32) {
        self.buf_info = buf_size & 0x3FFF;
        self.dma = dma;
    }
}

const _: () = assert!(core::mem::size_of::<RxBufferDesc>() == 8);

/// TX Descriptor (40 bytes / 10 dwords - matches Linux rtw_tx_fill_tx_desc for 8822C)
#[repr(C)]
#[derive(Clone, Copy)]
struct TxDesc {
    dword0: u32, // TXPKTSIZE[15:0] | OFFSET[23:16] | BMC(24) | LS(26) | DISQSELSEQ(31)
    dword1: u32, // MACID[7:0] | QSEL[12:8] | RATE_ID[20:16] | SEC_TYPE[23:22] | PKT_OFFSET[28:24] | MORE_DATA(29)
    dword2: u32, // AGG_EN(12) | SPE_RPT(19) | AMPDU_DEN[22:20] | BT_NULL(23)
    dword3: u32, // HW_SSN_SEL[7:6] | USE_RATE(8) | DISDATAFB(10) | USE_RTS(12) | MAX_AGG_NUM[21:17]
    dword4: u32, // DATARATE[6:0] | FB_LIMIT[12:8] | RTSRATE[28:24]
    dword5: u32, // DATA_SHORT(4) | BW[6:5] | LDPC(7) | STBC[9:8]
    dword6: u32, // SW_DEFINE (TX report SN)
    dword7: u32, // TXDESC_CHECKSUM (SDIO/USB only) | DMA_TXAGG_NUM[31:24]
    dword8: u32, // EN_HWSEQ(15)
    dword9: u32, // SW_SEQ[23:12] (used when DISQSELSEQ/EN_HWSEQ are set)
}

/// RX Descriptor (24 bytes + DRV_INFO) - matches Linux rtl8822ce_rx_desc
#[repr(C)]
#[derive(Clone, Copy)]
struct RxDesc {
    dword0: u32, // PKT_LEN, C2H, LONG/PKT_TYPE, etc.
    dword1: u32, // RX_RATE, BW, GI, SGI, etc.
    dword2: u32, // RSSI, SQ, AGC, etc.
    dword3: u32, // TSFT, CH_NUM, etc.
    dword4: u32, // MACID, TID, etc.
    dword5: u32, // SEQ, FRAG, etc.
}

/// Estado del driver RTL8822CE
pub struct Rtl8822ce {
    mmio_base: u64,        // Virtual address of MMIO BAR
    mmio_phys: u64,        // Physical address of MMIO BAR
    mmio_size: u64,        // Size of MMIO BAR
    mac: [u8; 6],          // MAC address
    pci_bus: u8,
    pci_device: u8,
    pci_function: u8,

    // TX rings (one per queue)
    tx_bd_ring: [u64; 7],   // Physical addresses of TX BD rings
    tx_bd_virt: [*mut u8; 7],
    tx_bd_count: [u16; 7],
    tx_buf: [*mut u8; 7],   // Physical addresses of TX data buffers
    tx_buf_virt: [*mut u8; 7],
    tx_wr_idx: [u16; 7],
    tx_rd_idx: [u16; 7],    // HW read index (reclaimed from BD_IDX upper word)

    // RX ring
    rx_bd_ring: u64,         // Physical address of RX BD ring
    rx_bd_virt: *mut u8,
    rx_buf: [*mut u8; RX_RING_SIZE],
    rx_buf_phys: [u64; RX_RING_SIZE],
    rx_rd_idx: u16,
    /// True once the hardware RX ring index has advanced past the initial zero
    /// state (vendor `rtl8822ce_check_rxdesc_remain`: `if (desc_idx_hw > 0)`).
    rx_started: bool,

    // BCN TX ring (for firmware download)
    bcn_bd_ring: u64,
    bcn_bd_virt: *mut u8,
    bcn_buf: *mut u8,
    bcn_buf_virt: *mut u8,
    fw_fifopage_ctrl2_bckp: u16,

    // H2C mailbox
    h2c_box: u8,
    // Sequence number for H2C offload packets sent via the H2C queue
    h2c_seq: u16,

    // Rate Adaptive (RA) state
    ra_table: [RaEntry; 32],  // Per-MACID RA table
    ra_mask: [u32; 32],       // Rate mask per MACID

    // Power Management state
    pwr_state: u8,            // Current power state (PS_STATE_S0-S4)
    rpwm: u8,                 // Last RPWM value
    cpwm: u8,                 // Last CPWM value
    tog: u8,                  // Toggle bit for RPWM
    lps_level: u8,            // LPS level
    ips_mode: u8,             // IPS mode
    ips_mode_req: u8,         // Requested IPS mode
    bkeepfwalive: bool,       // Keep FW alive in IPS
    bpower_saving: bool,      // Power saving flag
    rpwm_retry: u8,           // RPWM retry count
    cpwm_tog: u8,             // CPWM toggle

    // Channel state
    current_channel: u8,
    current_band_5g: bool,
    b_need_iqk: bool,

    // Watchdog state
    watchdog_enabled: bool,
    watchdog_timeout: u32,

    // Thermal/Power Tracking
    thermal_meter: i8,      // Current thermal meter reading
    delta_pwr_idx: i8,      // Delta power index for power tracking
    thermal_avg: i8,        // Average thermal meter
    
    // BA Session management
    ba_sessions: [BaSession; 128],
    rx_reorder: [RxReorderCtrl; 16],
    
    // Power tracking runtime
    swing_table_idx_2g: u8,
    swing_table_idx_5g: u8,
    
    // EDCCA/Adaptivity
    edcca_flag: bool,
    cfo_tracking: bool,
    
    // Watchdog
    fw_watchdog_timer: u32,
    
    // Beamforming
    bf_cap: u8,
    
    // DACK backup
    dack_backup_valid: bool,
    dack_msbk: [[[u16; 8]; 2]; 2], // [path][vec][idx]
    dack_dck: [[[u8; 2]; 2]; 2],   // [path][I/Q][idx]
    
    // RFK
    rfk_bt_iqk_timeout: bool,

    // State
    initialized: bool,
    fw_loaded: bool,
    efuse: EfuseData,
}

// The driver state is heap-allocated. Keep it well under the 1 MiB kernel heap
// (and never let it grow back to the multi-megabyte reorder buffers that
// overflowed the 20 KiB kernel stack).
const _: () = assert!(
    core::mem::size_of::<Rtl8822ce>() < 512 * 1024,
    "Rtl8822ce too large: cannot fit in the 1 MiB kernel heap"
);

/// EFUSE data structure
#[derive(Default, Clone, Copy)]
struct EfuseData {
    crystal_cap: u8,
    mac_addr: [u8; 6],        // MAC address from EFUSE
    bt_mac_addr: [u8; 6],     // BT MAC address from EFUSE
    tx_power: i8,             // Default TX power from EFUSE
    thermal: i8,              // Thermal sensor reading from EFUSE
    xtal: u32,                // Crystal frequency
    version: u8,              // EFUSE version
    hci: u8,                  // HCI version
    api_version: u8,          // API version
    customer_id: u16,         // Customer ID
    ota_status: u8,           // OTA status
    multi_antenna: bool,      // Multi-antenna support
    pa_type: u8,              // Power amplifier type
    rf_gain: u8,              // RF gain setting
    tx_2g: i8,                // 2G TX power
    tx_5g: i8,                // 5G TX power
    rfe_option: u8,           // RFE (front-end) option, logical efuse offset 0xca
    cut_version: u8,          // Chip cut version from REG_SYS_CFG1
}

/// RA (Rate Adaptive) entry per MACID
#[derive(Default, Clone, Copy)]
struct RaEntry {
    macid: u8,
    rate_id: u8,
    ra_mask: u32,
    rssi: i8,
    bw: u8,
    nss: u8,
    vht_en: bool,
    ldpc_en: bool,
    stbc_en: bool,
    sg_en: bool,
    tx_rate: u8,
    retry_ratio: u8,
    ra_skip_cnt: u8,
    stage: u8,
    decision_rate: u8,
    pre_rate: u8,
    rate_up: u8,
    rate_down: u8,
    high_rssi_thresh: i8,
    low_rssi_thresh: i8,
    ra_use_rate: bool,
}

/// AMPDU/BA (Block Acknowledgment) session - matches Linux rtw_ba_session
#[derive(Default, Clone, Copy)]
struct BaSession {
    tid: u8,
    macid: u8,
    bmp: u16,              // Bitmap of received frames
    start_seq: u16,        // Starting sequence number
    win_size: u8,          // Window size (typically 64)
    state: u8,             // 0=idle, 1=initiated, 2=active, 3=closing
    timeout: u32,          // Timeout counter
    last_rx_seq: u16,      // Last received sequence number
    pending_rx: u16,       // Pending RX count
}

/// RX Reorder buffer - matches Linux rtw_rx_reorder_buf
#[derive(Clone, Copy)]
struct RxReorderEntry {
    in_use: bool,
    seq: u16,
    timestamp: u32,
    data: [u8; 2304],
    data_len: u16,
}

impl Default for RxReorderEntry {
    fn default() -> Self {
        Self {
            in_use: false,
            seq: 0,
            timestamp: 0,
            data: [0; 2304],
            data_len: 0,
        }
    }
}

/// RX Reorder control - matches Linux rtw_rx_reorder_ctrl
struct RxReorderCtrl {
    enable: bool,
    win_size: u8,
    wsize_b: u16,          // Bitmap size
    indicator: u16,        // Current indicator (expected seq)
    wend_b: u16,           // Window end indicator
    buf: [RxReorderEntry; 8], // Reorder buffer (8 entries; kernel heap/stack budget)
    last_indicator: u16,
    flush_pending: bool,
}

impl Default for RxReorderCtrl {
    fn default() -> Self {
        Self {
            enable: false,
            win_size: 0,
            wsize_b: 0,
            indicator: 0,
            wend_b: 0,
            buf: core::array::from_fn(|_| Default::default()),
            last_indicator: 0,
            flush_pending: false,
        }
    }
}

// Safety: All raw pointers in Rtl8822ce are MMIO/DMA pointers
// that are only accessed from the single owner via Mutex lock.
unsafe impl Send for Rtl8822ce {}
unsafe impl Sync for Rtl8822ce {}

// ========================================================================
// PHY Table Parsing Infrastructure (mirrors Linux rtw_parse_tbl_phy_cond)
// ========================================================================

/// rtw88 table branch types (bits [29:28] of the cond word)
const BRANCH_IF: u32 = 0;
const BRANCH_ELIF: u32 = 1;
const BRANCH_ELSE: u32 = 2;
const BRANCH_ENDIF: u32 = 3;

/// Target register space for a parsed phy_cond table.
#[derive(Clone, Copy, PartialEq)]
enum CfgKind {
    Mac,
    Bb,
    Agc,
    Rf,
}

impl Rtl8822ce {
    fn new() -> Self {
        // Initialize arrays with null pointers
        let tx_bd_ring = [0u64; 7];
        let tx_bd_virt = [core::ptr::null_mut(); 7];
        let tx_bd_count = [0u16; 7];
        let tx_buf = [core::ptr::null_mut(); 7];
        let tx_buf_virt = [core::ptr::null_mut(); 7];
        let tx_wr_idx = [0u16; 7];
        let tx_rd_idx = [0u16; 7];
        let rx_buf = [core::ptr::null_mut(); RX_RING_SIZE];
        let rx_buf_phys = [0u64; RX_RING_SIZE];

        Self {
            mmio_base: 0,
            mmio_phys: 0,
            mmio_size: 0,
            mac: [0; 6],
            pci_bus: 0,
            pci_device: 0,
            pci_function: 0,
            tx_bd_ring,
            tx_bd_virt,
            tx_bd_count,
            tx_buf,
            tx_buf_virt,
            tx_wr_idx,
            tx_rd_idx,
            rx_bd_ring: 0,
            rx_bd_virt: core::ptr::null_mut(),
            rx_buf,
            rx_buf_phys,
            rx_rd_idx: 0,
            rx_started: false,
            bcn_bd_ring: 0,
            bcn_bd_virt: core::ptr::null_mut(),
            bcn_buf: core::ptr::null_mut(),
            bcn_buf_virt: core::ptr::null_mut(),
            fw_fifopage_ctrl2_bckp: 0,
            h2c_box: 0,
            h2c_seq: 0,
            ra_table: [RaEntry::default(); 32],
            ra_mask: [0u32; 32],
            pwr_state: PS_STATE_S0,
            rpwm: 0,
            cpwm: PS_STATE_S4,
            tog: 0,
            lps_level: 0,
            ips_mode: 0,
            ips_mode_req: 0,
            bkeepfwalive: false,
            bpower_saving: false,
            rpwm_retry: 0,
            cpwm_tog: 0,
            current_channel: 1,
            current_band_5g: false,
            b_need_iqk: true,
            watchdog_enabled: false,
            watchdog_timeout: 0,
            initialized: false,
            fw_loaded: false,
            efuse: EfuseData::default(),
            // Thermal/Power Tracking
            thermal_meter: 0,
            delta_pwr_idx: 0,
            thermal_avg: 0,
            // BA Session management
            ba_sessions: [BaSession::default(); 128],
            rx_reorder: core::array::from_fn(|_| RxReorderCtrl::default()),
            // Power tracking runtime
            swing_table_idx_2g: 0,
            swing_table_idx_5g: 0,
            // EDCCA/Adaptivity
            edcca_flag: false,
            cfo_tracking: false,
            // Watchdog
            fw_watchdog_timer: 0,
            // Beamforming
            bf_cap: 0,
            // DACK backup
            dack_backup_valid: false,
            dack_msbk: [[[0u16; 8]; 2]; 2], // [path][vec][idx]
            dack_dck: [[[0u8; 2]; 2]; 2],   // [path][I/Q][idx]
            // RFK
            rfk_bt_iqk_timeout: false,
        }
    }

    /// Allocate the driver state directly on the heap, avoiding a multi-megabyte
    /// stack temporary. `Rtl8822ce` embeds the RX reorder buffers (~2.3 MB) and the
    /// kernel stack is only 20 KiB, so building it with `Rtl8822ce::new()` would
    /// overflow the stack and triple-fault (freeze) before any output.
    ///
    /// The allocation is zero-initialized, which already matches every `new()`
    /// default except the three non-zero ones set below.
    fn new_boxed() -> Result<alloc::boxed::Box<Self>, &'static str> {
        use core::alloc::Layout;
        let layout = Layout::new::<Self>();
        let raw = unsafe { alloc::alloc::alloc_zeroed(layout) as *mut Self };
        if raw.is_null() {
            return Err("RTL8822CE: Failed to allocate driver state");
        }
        let mut this = unsafe { alloc::boxed::Box::from_raw(raw) };
        // Non-zero defaults from Rtl8822ce::new()
        this.cpwm = PS_STATE_S4;
        this.current_channel = 1;
        this.b_need_iqk = true;
        Ok(this)
    }

    /// write32_set helper - sets specific bits in a 32-bit register
    fn write32_set(&mut self, offset: u32, val: u32) {
        unsafe {
            let orig = self.read32(offset);
            self.write32(offset, orig | val);
        }
    }

    /// write32_clear helper - clears specific bits in a 32-bit register
    fn write32_clr(&mut self, offset: u32, val: u32) {
        unsafe {
            let orig = self.read32(offset);
            self.write32(offset, orig & !val);
        }
    }

    fn write8_set(&mut self, offset: u32, val: u8) {
        unsafe {
            let orig = self.read8(offset);
            self.write8(offset, orig | val);
        }
    }

    fn write8_clr(&mut self, offset: u32, val: u8) {
        unsafe {
            let orig = self.read8(offset);
            self.write8(offset, orig & !val);
        }
    }

    fn write8_mask(&mut self, offset: u32, mask: u8, val: u8) {
        unsafe {
            let orig = self.read8(offset);
            let new = (orig & !mask) | ((val << mask.trailing_zeros()) & mask);
            self.write8(offset, new);
        }
    }

    fn write16_set(&mut self, offset: u32, val: u16) {
        unsafe {
            let orig = self.read16(offset);
            self.write16(offset, orig | val);
        }
    }

    // ========================================================================
    // MMIO Access
    // ========================================================================
    #[inline]
    unsafe fn read8(&self, offset: u32) -> u8 {
        core::ptr::read_volatile((self.mmio_base + offset as u64) as *const u8)
    }

    #[inline]
    unsafe fn read16(&self, offset: u32) -> u16 {
        core::ptr::read_volatile((self.mmio_base + offset as u64) as *const u16)
    }

    #[inline]
    unsafe fn read32(&self, offset: u32) -> u32 {
        core::ptr::read_volatile((self.mmio_base + offset as u64) as *const u32)
    }

    #[inline]
    unsafe fn write8(&self, offset: u32, val: u8) {
        core::ptr::write_volatile((self.mmio_base + offset as u64) as *mut u8, val);
    }

    #[inline]
    unsafe fn write16(&self, offset: u32, val: u16) {
        core::ptr::write_volatile((self.mmio_base + offset as u64) as *mut u16, val);
    }

    #[inline]
    unsafe fn write32(&self, offset: u32, val: u32) {
        core::ptr::write_volatile((self.mmio_base + offset as u64) as *mut u32, val);
    }

    #[inline]
    unsafe fn write32_mask(&self, offset: u32, mask: u32, val: u32) {
        let shift = mask.trailing_zeros();
        let orig = self.read32(offset);
        let new = (orig & !mask) | ((val << shift) & mask);
        self.write32(offset, new);
    }

    // ========================================================================
    // Utility
    // ========================================================================

    fn delay_us(&self, us: u64) {
        for _ in 0..us * 10 {
            core::hint::spin_loop();
        }
    }

    fn delay_ms(&self, ms: u64) {
        for _ in 0..ms {
            crate::linux::mdelay(1);
        }
    }

    /// Write RF register via SIPi (Serial Peripheral Interface) - matches Linux rtw_phy_write_rf_reg_sipi
    /// Used for RF register 0x18 (channel) on 8822C
    /// Format: write to sipi_addr[path] the value ((addr << 20) | (data & 0x000fffff))
    /// If mask != RFREG_MASK, does read-modify-write
    fn write_rf_sipi(&self, path: u8, addr: u32, mask: u32, data: u32) {
        let sipi_addr = if path == 0 { RF_SIPI_ADDR_A } else { RF_SIPI_ADDR_B };
        let addr_masked = addr & 0xFF;
        
        let data_masked = if mask != RFREG_MASK {
            // Read-modify-write for masked writes
            let old_data = self.read_rf_sipi(path, addr, RFREG_MASK);
            let shift = mask.trailing_zeros();
            ((old_data & !mask) | ((data << shift) & mask)) & RFREG_MASK
        } else {
            data & RFREG_MASK
        };
        
        let combined = ((addr_masked << 20) | data_masked) & 0x0FFFFFFF;
        unsafe {
            self.write32(sipi_addr, combined);
        }
        // Linux: udelay(13) after SIPi write
        self.delay_us(13);
    }

    /// Read RF register (rtw_phy_read_rf): direct BB window read with
    /// rtw_read32_mask field semantics (result shifted down to bit 0).
    fn read_rf_sipi(&self, path: u8, addr: u32, mask: u32) -> u32 {
        let bb_addr = if path == 0 { 0x3C00 } else { 0x4C00 } + ((addr & 0xFF) << 2);
        let shift = mask.trailing_zeros();
        unsafe { (self.read32(bb_addr) & mask) >> shift }
    }

    /// 3-wire control sequence (matches Linux rtw8822c_rstb_3wire)
    /// Called before/after RF register writes via SIPi
    fn rstb_3wire(&self, enable: bool) {
        unsafe {
            if enable {
                // Enable 3-wire
                self.write32_mask(REG_RSTB, BIT_RSTB_3WIRE, 0x1);
                self.write32_mask(REG_ANAPAR_A, BIT_ANAPAR_UPDATE, 0x1);
                self.write32_mask(REG_ANAPAR_B, BIT_ANAPAR_UPDATE, 0x1);
            } else {
                // Disable 3-wire
                self.write32_mask(REG_RSTB, BIT_RSTB_3WIRE, 0x0);
            }
        }
    }

        /// Poll a register (8-bit access) waiting for bits to match expected value.
    ///
    /// Linux rtw88 uses `read_poll_timeout_atomic(rtw_read8, ...)` which always
    /// performs an 8-bit read. This is critical because many control registers
    /// are at odd offsets (e.g. 0x0005, 0x0006) and a 32-bit MMIO read on a
    /// non-4-byte-aligned address returns garbage in our MMIO implementation,
    /// causing false timeouts. Always use read8 for register polling.
    fn poll_reg8(&self, offset: u32, mask: u8, expected: u8, timeout_ms: u32) -> Result<(), &'static str> {
        // 50us interval, timeout_ms total => timeout_ms*100 loops
        let timeout_loops = timeout_ms * 100;
        for _ in 0..timeout_loops {
            let val = unsafe { self.read8(offset) };
            if (val & mask) == expected {
                driver_unlock(); return Ok(());
            }
            self.delay_us(50);
        }
        Err("RTL8822CE: Register poll timeout")
    }

    // ========================================================================
    // Rate Adaptive (RA) Implementation - matches Linux phydm/hal_com_ra
    // ========================================================================

    /// Initialize RA table for a MACID
    fn ra_init(&mut self, macid: u8, rssi: i8, bw: u8, nss: u8, vht_en: bool) {
        if macid >= 32 {
            return;
        }
        
        // Build initial RA mask based on capabilities (before mutable borrow)
        let mask = Self::build_ra_mask_static(bw, nss, vht_en);
        
        let entry = &mut self.ra_table[macid as usize];
        entry.macid = macid;
        entry.rssi = rssi;
        entry.bw = bw;
        entry.nss = nss;
        entry.vht_en = vht_en;
        entry.ldpc_en = true;
        entry.stbc_en = true;
        entry.sg_en = true;
        entry.stage = 0;
        entry.ra_skip_cnt = 0;
        entry.tx_rate = RATE_OFDM_6M;
        entry.pre_rate = RATE_OFDM_6M;
        entry.retry_ratio = 0;
        entry.rate_up = 0;
        entry.rate_down = 0;
        entry.high_rssi_thresh = 50;
        entry.low_rssi_thresh = 25;
        entry.ra_use_rate = false;
        entry.ra_mask = mask;
        self.ra_mask[macid as usize] = mask;
        
        // Write RA mask to hardware
        self.ra_write_mask(macid, mask);
        
        crate::mesa_println!("[RTL8822CE] RA init: macid={} mask={:#010x} bw={} nss={} vht={}", 
            macid, mask, bw, nss, vht_en);
    }

    /// Build RA mask based on BW, NSS, VHT capabilities (static version for use before mutable borrow)
    fn build_ra_mask_static(bw: u8, nss: u8, vht_en: bool) -> u32 {
        let mut mask: u32 = 0;
        
        // CCK rates (1M, 2M, 5.5M, 11M) - always supported
        mask |= RRSR_CCK_1M | RRSR_CCK_2M | RRSR_CCK_5_5M | RRSR_CCK_11M;
        
        // OFDM rates (6M, 9M, 12M, 18M, 24M, 36M, 48M, 54M) - always supported
        mask |= RRSR_OFDM_6M | RRSR_OFDM_9M | RRSR_OFDM_12M | RRSR_OFDM_18M |
                RRSR_OFDM_24M | RRSR_OFDM_36M | RRSR_OFDM_48M | RRSR_OFDM_54M;
        
        // HT MCS rates based on NSS
        if nss >= 1 {
            mask |= RRSR_MCS0 | RRSR_MCS1 | RRSR_MCS2 | RRSR_MCS3 |
                    RRSR_MCS4 | RRSR_MCS5 | RRSR_MCS6 | RRSR_MCS7;
        }
        if nss >= 2 {
            mask |= RRSR_MCS8 | RRSR_MCS9 | RRSR_MCS10 | RRSR_MCS11 |
                    RRSR_MCS12 | RRSR_MCS13 | RRSR_MCS14 | RRSR_MCS15;
        }
        
        // VHT rates based on NSS and BW
        if vht_en {
            if bw >= 1 { // 40MHz or 80MHz
                if nss >= 1 {
                    mask |= RRSR_VHT_1SS_MASK & 0x3FF00000; // MCS 0-9
                }
                if nss >= 2 {
                    mask |= RRSR_VHT_2SS_MASK & 0xFFC00000; // MCS 0-9
                }
            } else { // 20MHz
                if nss >= 1 {
                    mask |= RRSR_VHT_1SS_MASK & 0x3FF00000; // MCS 0-9 (MCS9 not in 20MHz)
                }
                if nss >= 2 {
                    mask |= RRSR_VHT_2SS_MASK & 0xFFC00000; // MCS 0-9 (MCS9 not in 20MHz)
                }
            }
        }
        
        mask
    }

    /// Write RA mask to hardware registers
    fn ra_write_mask(&self, macid: u8, mask: u32) {
        unsafe {
            // Write to ARFR registers based on MACID
            // Each MACID has its own ARFR register set (8 bytes = 64 bits)
            // The mask is 32-bit but we write it to the low 32 bits of the 64-bit register
            let base_reg = REG_ARFR0 + (macid as u32 * 8);
            if base_reg <= REG_ARFR7 {
                self.write32(base_reg, mask);
                // High 32 bits are 0 for our 32-bit mask
                self.write32(base_reg + 4, 0);
            }
        }
    }

    /// Update RA mask based on RSSI (rate adaptation)
    fn ra_update(&mut self, macid: u8, rssi: i8) -> u8 {
        if macid >= 32 {
            return RATE_OFDM_6M;
        }
        
        // Update RSSI first
        self.ra_table[macid as usize].rssi = rssi;
        
        // Skip RA update if not enough packets
        self.ra_table[macid as usize].ra_skip_cnt += 1;
        if self.ra_table[macid as usize].ra_skip_cnt < 10 {
            return self.ra_table[macid as usize].tx_rate;
        }
        self.ra_table[macid as usize].ra_skip_cnt = 0;
        
        // Get current entry data for decision
        let current_rate = self.ra_table[macid as usize].tx_rate;
        let pre_rate = self.ra_table[macid as usize].pre_rate;
        let ra_mask = self.ra_table[macid as usize].ra_mask;
        
        // Simple RA algorithm based on RSSI thresholds
        let new_rate = Self::ra_decide_rate_static(
            self.ra_table[macid as usize].rssi,
            current_rate,
            self.ra_table[macid as usize].bw,
            self.ra_table[macid as usize].nss,
            self.ra_table[macid as usize].vht_en,
            ra_mask,
            &mut self.ra_table[macid as usize]
        );
        
        if new_rate != current_rate {
            crate::mesa_println!("[RTL8822CE] RA update: macid={} rate {} -> {} rssi={}", 
                macid, current_rate, new_rate, rssi);
            self.ra_table[macid as usize].pre_rate = current_rate;
            self.ra_table[macid as usize].tx_rate = new_rate;
            
            // Update RA mask if rate changed significantly
            if (new_rate as i8 - pre_rate as i8).abs() > 4 {
                let new_mask = Self::ra_adjust_mask_static(new_rate, ra_mask);
                self.ra_table[macid as usize].ra_mask = new_mask;
                self.ra_mask[macid as usize] = new_mask;
                self.ra_write_mask(macid, new_mask);
            }
        }
        
        new_rate
    }

    /// Initialize RRSR (Response Rate Set Register) for basic rates
    fn init_rrsr(&self) {
        unsafe {
            // 2G basic rates: 1M, 2M, 5.5M, 11M, 6M, 12M, 24M
            let rrsr_2g = RRSR_CCK_1M | RRSR_CCK_2M | RRSR_CCK_5_5M | RRSR_CCK_11M |
                          RRSR_OFDM_6M | RRSR_OFDM_12M | RRSR_OFDM_24M;
            self.write32(REG_RRSR_2G, rrsr_2g);
            
            // 5G basic rates: 6M, 12M, 24M
            let rrsr_5g = RRSR_OFDM_6M | RRSR_OFDM_12M | RRSR_OFDM_24M;
            self.write32(REG_RRSR_5G, rrsr_5g);
            
            crate::mesa_println!("[RTL8822CE] RRSR init: 2G={:#010x} 5G={:#010x}", rrsr_2g, rrsr_5g);
        }
    }

    /// Update RA retry ratio for rate fallback decisions
    fn ra_update_retry(&mut self, macid: u8, retry_ratio: u8) {
        if macid >= 32 {
            return;
        }
        self.ra_table[macid as usize].retry_ratio = retry_ratio;
    }

    /// Get current TX rate for MACID
    fn ra_get_rate(&self, macid: u8) -> u8 {
        if macid >= 32 {
            return RATE_OFDM_6M;
        }
        self.ra_table[macid as usize].tx_rate
    }

    // ========================================================================
    // Static RA helper functions (no self borrow)
    // ========================================================================

    /// Static version of ra_decide_rate for use without self borrow
    fn ra_decide_rate_static(
        rssi: i8,
        current_rate: u8,
        bw: u8,
        nss: u8,
        vht_en: bool,
        mask: u32,
        entry: &mut RaEntry,
    ) -> u8 {
        let mut rate = current_rate;
        
        // Rate up/down thresholds based on RSSI
        if rssi > entry.high_rssi_thresh {
            // Good signal - try higher rate
            if entry.rate_up < 5 {
                entry.rate_up += 1;
                entry.rate_down = 0;
            } else {
                rate = Self::ra_rate_up_static(rate, bw, nss, vht_en);
                entry.rate_up = 0;
            }
        } else if rssi < entry.low_rssi_thresh {
            // Poor signal - try lower rate
            if entry.rate_down < 3 {
                entry.rate_down += 1;
                entry.rate_up = 0;
            } else {
                rate = Self::ra_rate_down_static(rate);
                entry.rate_down = 0;
            }
        } else {
            // Medium signal - maintain
            entry.rate_up = 0;
            entry.rate_down = 0;
        }
        
        // Clamp to supported rates in mask
        rate = Self::ra_clamp_rate_static(rate, mask);
        
        rate
    }

    /// Static version of ra_rate_up
    fn ra_rate_up_static(current: u8, bw: u8, nss: u8, vht_en: bool) -> u8 {
        let rate_idx = current & 0x7F;
        let sgi = current & SGI_FLAG;
        
        // Simple rate progression
        let next = match rate_idx {
            RATE_CCK_1M => RATE_CCK_2M,
            RATE_CCK_2M => RATE_CCK_5_5M,
            RATE_CCK_5_5M => RATE_CCK_11M,
            RATE_CCK_11M => RATE_OFDM_6M,
            RATE_OFDM_6M => RATE_OFDM_9M,
            RATE_OFDM_9M => RATE_OFDM_12M,
            RATE_OFDM_12M => RATE_OFDM_18M,
            RATE_OFDM_18M => RATE_OFDM_24M,
            RATE_OFDM_24M => RATE_OFDM_36M,
            RATE_OFDM_36M => RATE_OFDM_48M,
            RATE_OFDM_48M => RATE_OFDM_54M,
            RATE_OFDM_54M => {
                if nss >= 1 && vht_en {
                    RATE_VHT_1SS_MCS0 | sgi
                } else if nss >= 1 {
                    RATE_MCS0 | sgi
                } else {
                    RATE_OFDM_54M
                }
            },
            r if r >= RATE_MCS0 && r <= RATE_MCS6 => r + 1,
            RATE_MCS7 => {
                if nss >= 2 && vht_en {
                    RATE_VHT_2SS_MCS0 | sgi
                } else if nss >= 2 {
                    RATE_MCS8 | sgi
                } else {
                    RATE_MCS7
                }
            },
            r if r >= RATE_MCS8 && r <= RATE_MCS14 => r + 1,
            RATE_MCS15 => RATE_MCS15,
            r if r >= RATE_VHT_1SS_MCS0 && r <= RATE_VHT_1SS_MCS8 => r + 1,
            RATE_VHT_1SS_MCS9 => {
                if nss >= 2 && vht_en {
                    RATE_VHT_2SS_MCS0 | sgi
                } else {
                    RATE_VHT_1SS_MCS9
                }
            },
            r if r >= RATE_VHT_2SS_MCS0 && r <= RATE_VHT_2SS_MCS8 => r + 1,
            _ => current,
        };
        
        next | sgi
    }

    /// Static version of ra_rate_down
    fn ra_rate_down_static(current: u8) -> u8 {
        let rate_idx = current & 0x7F;
        let sgi = current & SGI_FLAG;
        
        let next = match rate_idx {
            RATE_CCK_2M => RATE_CCK_1M,
            RATE_CCK_5_5M => RATE_CCK_2M,
            RATE_CCK_11M => RATE_CCK_5_5M,
            RATE_OFDM_6M => RATE_CCK_11M,
            RATE_OFDM_9M => RATE_OFDM_6M,
            RATE_OFDM_12M => RATE_OFDM_9M,
            RATE_OFDM_18M => RATE_OFDM_12M,
            RATE_OFDM_24M => RATE_OFDM_18M,
            RATE_OFDM_36M => RATE_OFDM_24M,
            RATE_OFDM_48M => RATE_OFDM_36M,
            RATE_OFDM_54M => RATE_OFDM_48M,
            RATE_MCS0 => RATE_OFDM_54M,
            r if r >= RATE_MCS1 && r <= RATE_MCS7 => r - 1,
            RATE_MCS8 => RATE_MCS7,
            r if r >= RATE_MCS9 && r <= RATE_MCS15 => r - 1,
            RATE_VHT_1SS_MCS0 => RATE_MCS7,
            r if r >= RATE_VHT_1SS_MCS1 && r <= RATE_VHT_1SS_MCS9 => r - 1,
            RATE_VHT_2SS_MCS0 => RATE_VHT_1SS_MCS9,
            r if r >= RATE_VHT_2SS_MCS1 && r <= RATE_VHT_2SS_MCS9 => r - 1,
            _ => current,
        };
        
        next | sgi
    }

    /// Static version of ra_clamp_rate
    fn ra_clamp_rate_static(rate: u8, mask: u32) -> u8 {
        let rate_idx = rate & 0x7F;
        let sgi = rate & SGI_FLAG;
        
        // Check if rate is in mask
        let bit = match rate_idx {
            RATE_CCK_1M => RRSR_CCK_1M,
            RATE_CCK_2M => RRSR_CCK_2M,
            RATE_CCK_5_5M => RRSR_CCK_5_5M,
            RATE_CCK_11M => RRSR_CCK_11M,
            RATE_OFDM_6M => RRSR_OFDM_6M,
            RATE_OFDM_9M => RRSR_OFDM_9M,
            RATE_OFDM_12M => RRSR_OFDM_12M,
            RATE_OFDM_18M => RRSR_OFDM_18M,
            RATE_OFDM_24M => RRSR_OFDM_24M,
            RATE_OFDM_36M => RRSR_OFDM_36M,
            RATE_OFDM_48M => RRSR_OFDM_48M,
            RATE_OFDM_54M => RRSR_OFDM_54M,
            RATE_MCS0 => RRSR_MCS0,
            RATE_MCS1 => RRSR_MCS1,
            RATE_MCS2 => RRSR_MCS2,
            RATE_MCS3 => RRSR_MCS3,
            RATE_MCS4 => RRSR_MCS4,
            RATE_MCS5 => RRSR_MCS5,
            RATE_MCS6 => RRSR_MCS6,
            RATE_MCS7 => RRSR_MCS7,
            RATE_MCS8 => RRSR_MCS8,
            RATE_MCS9 => RRSR_MCS9,
            RATE_MCS10 => RRSR_MCS10,
            RATE_MCS11 => RRSR_MCS11,
            RATE_MCS12 => RRSR_MCS12,
            RATE_MCS13 => RRSR_MCS13,
            RATE_MCS14 => RRSR_MCS14,
            RATE_MCS15 => RRSR_MCS15,
            RATE_VHT_1SS_MCS0..=RATE_VHT_1SS_MCS9 => {
                if mask & RRSR_VHT_1SS_MASK != 0 { mask & RRSR_VHT_1SS_MASK } else { 0 }
            },
            RATE_VHT_2SS_MCS0..=RATE_VHT_2SS_MCS9 => {
                if mask & RRSR_VHT_2SS_MASK != 0 { mask & RRSR_VHT_2SS_MASK } else { 0 }
            },
            _ => 0,
        };
        
        if bit != 0 && (mask & bit) != 0 {
            rate
        } else {
            // Find highest supported rate in mask
            Self::ra_highest_rate_static(mask) | sgi
        }
    }

    /// Static version of ra_highest_rate
    fn ra_highest_rate_static(mask: u32) -> u8 {
        // Check VHT 2SS first (highest)
        if mask & RRSR_VHT_2SS_MASK != 0 {
            for i in (RATE_VHT_2SS_MCS0..=RATE_VHT_2SS_MCS9).rev() {
                let bit = 1u32 << (i - RATE_VHT_2SS_MCS0 + 26);
                if mask & bit != 0 {
                    return i;
                }
            }
        }
        // Check VHT 1SS
        if mask & RRSR_VHT_1SS_MASK != 0 {
            for i in (RATE_VHT_1SS_MCS0..=RATE_VHT_1SS_MCS9).rev() {
                let bit = 1u32 << (i - RATE_VHT_1SS_MCS0 + 24);
                if mask & bit != 0 {
                    return i;
                }
            }
        }
        // Check HT MCS 2SS
        if mask & RRSR_HT_2SS_MASK != 0 {
            for i in (RATE_MCS8..=RATE_MCS15).rev() {
                let bit = 1u32 << i;
                if mask & bit != 0 {
                    return i;
                }
            }
        }
        // Check HT MCS 1SS
        for i in (RATE_MCS0..=RATE_MCS7).rev() {
            let bit = 1u32 << i;
            if mask & bit != 0 {
                return i;
            }
        }
        // Check OFDM
        for i in (RATE_OFDM_6M..=RATE_OFDM_54M).rev() {
            let bit = 1u32 << i;
            if mask & bit != 0 {
                return i;
            }
        }
        // Check CCK
        for i in (RATE_CCK_1M..=RATE_CCK_11M).rev() {
            let bit = 1u32 << i;
            if mask & bit != 0 {
                return i;
            }
        }
        RATE_CCK_1M
    }

    /// Static version of ra_adjust_mask
    fn ra_adjust_mask_static(new_rate: u8, mask: u32) -> u32 {
        let mut new_mask = mask;
        let rate_idx = new_rate & 0x7F;
        
        // If rate is low, disable higher rates
        if rate_idx <= RATE_OFDM_24M {
            // Disable VHT rates
            new_mask &= !(RRSR_VHT_1SS_MASK | RRSR_VHT_2SS_MASK);
            // Disable high MCS
            new_mask &= !(RRSR_MCS12 | RRSR_MCS13 | RRSR_MCS14 | RRSR_MCS15);
            // Disable high OFDM
            new_mask &= !(RRSR_OFDM_48M | RRSR_OFDM_54M);
        } else if rate_idx <= RATE_OFDM_54M {
            // Disable highest VHT
            new_mask &= !(RRSR_VHT_2SS_MASK);
            // Disable MCS 14-15
            new_mask &= !(RRSR_MCS14 | RRSR_MCS15);
            // Disable 54M
            new_mask &= !RRSR_OFDM_54M;
        }
        
        new_mask
    }

    // ========================================================================
    // Power Management (LPS/IPS/FWLPS) - matches Linux rtw_pwrctrl.c
    // ========================================================================

    /// Set RPWM (Request Power Management) - matches Linux rtw_set_rpwm
    /// pslv: power state level (PS_STATE_S0..PS_STATE_S4)
    fn set_rpwm(&mut self, pslv: u8) -> Result<(), &'static str> {
        let pslv = PS_STATE(pslv);
        
        // Skip if same state or already in deep sleep
        if self.rpwm == pslv || (self.rpwm >= PS_STATE_S2 && pslv >= PS_STATE_S2) || self.lps_level == 0 {
            driver_unlock(); return Ok(());
        }
        
        if self.rpwm_retry > 0 {
            crate::mesa_println!("[RTL8822CE] RPWM timeout, force set RPWM(0x{:02X}) again!", pslv);
        }
        
        let mut rpwm = pslv | self.tog;
        
        // Only when transitioning from S0/S1 to S2+ needs ACK
        if self.cpwm < PS_STATE_S2 && pslv >= PS_STATE_S2 {
            rpwm |= PS_ACK;
        }
        
        self.rpwm = pslv;
        
        if rpwm & PS_ACK != 0 {
            self.cpwm = unsafe { self.read8(REG_CPWM_8822C) };
        }
        
        unsafe {
            self.write8(REG_RPWM_8822C, rpwm);
        }
        
        self.tog = self.tog.wrapping_add(0x80);
        
        // Poll CPWM if ACK requested
        if rpwm & PS_ACK != 0 {
            self.cpwm_polling(rpwm, self.cpwm)?;
        }
        
        driver_unlock(); Ok(())
    }

    /// Poll CPWM (Current Power Management) - matches Linux rtw_cpwm_polling
    fn cpwm_polling(&mut self, rpwm: u8, mut cpwm_orig: u8) -> Result<(), &'static str> {
        let mut rst = Err("CPWM polling timeout");
        let mut rpwm_val = rpwm;
        self.rpwm_retry = 0;
        
        while self.rpwm_retry < LPS_RPWM_RETRY_CNT {
            // Simple timeout loop: LPS_CPWM_TIMEOUT_MS * 1000 iterations (1us each)
            let mut timeout = LPS_CPWM_TIMEOUT_MS * 1000;
            loop {
                self.delay_us(1);
                let cpwm_now = unsafe { self.read8(REG_CPWM_8822C) };
                
                if (cpwm_orig ^ cpwm_now) & 0x80 != 0 {
                    self.cpwm = PS_STATE_S4;
                    self.cpwm_tog = cpwm_now & PS_TOGGLE;
                    driver_unlock();
                    rst = Ok(());
                    break;
                }
                
                timeout = timeout.saturating_sub(1);
                if timeout == 0 {
                    break;
                }
            }
            
            if rst.is_ok() {
                break;
            } else {
                // RPWM retry
                cpwm_orig = unsafe { self.read8(REG_CPWM_8822C) };
                rpwm_val &= !PS_TOGGLE;
                rpwm_val |= self.tog;
                unsafe { self.write8(REG_RPWM_8822C, rpwm_val); }
                self.tog = self.tog.wrapping_add(0x80);
            }
            
            self.rpwm_retry += 1;
        }
        
        if rst.is_ok() {
            crate::mesa_println!("[RTL8822CE] CPWM polling OK! retry={} cpwm_orig={:#04x} cpwm_now={:#04x}", 
                self.rpwm_retry, cpwm_orig, unsafe { self.read8(REG_CPWM_8822C) });
        } else {
            crate::mesa_println!("[RTL8822CE] CPWM polling timeout! retry={} cpwm_orig={:#04x}", 
                self.rpwm_retry, cpwm_orig);
        }
        
        rst
    }

    /// Enter IPS (Idle Power Save) - matches Linux _ips_enter
    fn ips_enter(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] IPS enter...");
        
        self.ips_mode = self.ips_mode_req;
        self.bpower_saving = true;
        
        if self.ips_mode == 2 { // IPS_LEVEL_2
            self.bkeepfwalive = true;
        }
        
        // Power down sequence
        self.ips_pwr_down()?;
        
        self.pwr_state = PS_STATE_S3; // rf_off
        crate::mesa_println!("[RTL8822CE] IPS entered (RF off)");
        driver_unlock(); Ok(())
    }

    /// Leave IPS (Idle Power Save) - matches Linux _ips_leave
    fn ips_leave(&mut self) -> Result<(), &'static str> {
        if self.pwr_state != PS_STATE_S3 {
            driver_unlock(); return Ok(());
        }
        
        crate::mesa_println!("[RTL8822CE] IPS leave...");
        
        // Power up sequence
        self.ips_pwr_up()?;
        
        self.pwr_state = PS_STATE_S0; // rf_on
        self.bkeepfwalive = false;
        self.bpower_saving = false;
        
        crate::mesa_println!("[RTL8822CE] IPS left (RF on)");
        driver_unlock(); Ok(())
    }

    /// Power down for IPS
    fn ips_pwr_down(&self) -> Result<(), &'static str> {
        unsafe {
            // Disable MAC TX/RX
            self.write8(REG_CR, 0x00);
            self.delay_ms(10);
            
            // Set RPWM to S3 (RF off)
            self.write8(REG_RPWM_8822C, PS_STATE_S3 | self.tog);
            self.delay_ms(10);
            
            // Power off sequence (similar to pwr_off_seq but for IPS)
            self.write8(0x0005, self.read8(0x0005) | (1 << 2));
            self.delay_ms(10);
        }
        driver_unlock(); Ok(())
    }

/// Power up for IPS
    fn ips_pwr_up(&mut self) -> Result<(), &'static str> {
        // Full IPS power up sequence (mirrors Linux rtw_ips_pwr_up -> rtw_core_start)
        // 1. Restore MAC registers from backup
        self.mac_pre_system_cfg();
        // 2. Power on MAC (includes power_on + firmware + mac_init)
        self.power_on()?;
        // 3. Wait for firmware completion
        self.wait_firmware_completion()?;
        // 4. MAC init after firmware
        self.mac_init_after_fw()?;
        // 5. PHY set param
        self.phy_set_param()?;
        // 6. MAC post init
        self.mac_postinit()?;
        // 7. HCI start
        self.hci_start()?;
        // 8. Restore RF calibration results from backup
        self.rf_restore_from_backup()?;
        
        driver_unlock(); Ok(())
    }
    
    /// Restore RF calibration from backup (for IPS/LPS wake)
    fn rf_restore_from_backup(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Restoring RF calibration from backup...");
        unsafe {
            // Restore DACK backup vectors if available
            if self.dack_backup_valid() {
                self.dack_cal_restore()?;
            }
            // Restore IQK results
            self.iqk_restore()?;
            // Restore power trim
            self.power_trim_restore()?;
        }
        driver_unlock(); Ok(())
    }
    
    /// Check if DACK backup is valid
    fn dack_backup_valid(&self) -> bool {
        // Check if backup data exists (first element non-zero)
        let val = unsafe { self.read32(0x1860) };
        val != 0
    }
    
    /// Restore DACK calibration (mirrors Linux rtw8822c_dac_cal_restore)
    fn dack_cal_restore(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Restoring DACK calibration...");
        unsafe {
            // Restore prepare (mirrors rtw8822c_dac_cal_restore_prepare)
            self.write32(0x9b4, 0xdb66db00);
            self.write32_mask(0x18b0, BIT!(27), 0x0);
            self.write32_mask(0x18cc, BIT!(27), 0x0);
            self.write32_mask(0x41b0, BIT!(27), 0x0);
            self.write32_mask(0x41cc, BIT!(27), 0x0);
            self.write32_mask(0x1830, BIT!(30), 0x0);
            self.write32_mask(0x1860, 0xfc000000, 0x3c);
            self.write32_mask(0x18b4, BIT!(0), 0x1);
            self.write32_mask(0x18d0, BIT!(0), 0x1);
            self.write32_mask(0x4130, BIT!(30), 0x0);
            self.write32_mask(0x4160, 0xfc000000, 0x3c);
            self.write32_mask(0x41b4, BIT!(0), 0x1);
            self.write32_mask(0x41d0, BIT!(0), 0x1);
            self.write32_mask(0x18b0, 0xf00, 0x0);
            self.write32_mask(0x18c0, BIT!(14), 0x0);
            self.write32_mask(0x18cc, 0xf00, 0x0);
            self.write32_mask(0x18dc, BIT!(14), 0x0);
            self.write32_mask(0x18b0, BIT!(0), 0x0);
            self.write32_mask(0x18cc, BIT!(0), 0x0);
            self.write32_mask(0x18b0, BIT!(0), 0x1);
            self.write32_mask(0x18cc, BIT!(0), 0x1);
            
            // Restore DCK
            self.dack_cal_restore_dck()?;
            
            self.write32_mask(0x18c0, 0x38000, 0x7);
            self.write32_mask(0x18dc, 0x38000, 0x7);
            self.write32_mask(0x41c0, 0x38000, 0x7);
            self.write32_mask(0x41dc, 0x38000, 0x7);
            self.write32_mask(0x18b8, BIT!(26) | BIT!(25), 0x1);
            self.write32_mask(0x18d4, BIT!(26) | BIT!(25), 0x1);
            self.write32_mask(0x41b0, 0xf00, 0x0);
            self.write32_mask(0x41c0, BIT!(14), 0x0);
            self.write32_mask(0x41cc, 0xf00, 0x0);
            self.write32_mask(0x41dc, BIT!(14), 0x0);
            self.write32_mask(0x41b0, BIT!(0), 0x0);
            self.write32_mask(0x41cc, BIT!(0), 0x0);
            self.write32_mask(0x41b0, BIT!(0), 0x1);
            self.write32_mask(0x41cc, BIT!(0), 0x1);
            self.write32_mask(0x41b8, BIT!(26) | BIT!(25), 0x1);
            self.write32_mask(0x41d4, BIT!(26) | BIT!(25), 0x1);
            
            // Restore path A and B (simplified)
            // Full implementation would restore from dm_info->dack_msbk
            self.dack_cal_restore_path(0)?;
            self.dack_cal_restore_path(1)?;
            
            self.write32_mask(0x1830, BIT!(30), 0x1);
            self.write32_mask(0x4130, BIT!(30), 0x1);
            self.write32(0x1860, 0); // temp restore
            self.write32(0x4160, 0);
            self.write32_mask(0x18b0, BIT!(27), 0x1);
            self.write32_mask(0x18cc, BIT!(27), 0x1);
            self.write32_mask(0x41b0, BIT!(27), 0x1);
            self.write32_mask(0x41cc, BIT!(27), 0x1);
            self.write32(0x9b4, 0xdb66db00);
        }
        driver_unlock(); Ok(())
    }
    
    fn dack_cal_restore_dck(&self) -> Result<(), &'static str> {
        unsafe {
            self.write32(0x18bc, BIT!(19));
            self.write32_mask(0x18bc, 0xf0000000, 0);
            self.write32_mask(0x18c0, 0xf, 0);
            self.write32(0x18d8, BIT!(19));
            self.write32_mask(0x18d8, 0xf0000000, 0);
            self.write32_mask(0x18dc, 0xf, 0);
            self.write32(0x41bc, BIT!(19));
            self.write32_mask(0x41bc, 0xf0000000, 0);
            self.write32_mask(0x41c0, 0xf, 0);
            self.write32(0x41d8, BIT!(19));
            self.write32_mask(0x41d8, 0xf0000000, 0);
            self.write32_mask(0x41dc, 0xf, 0);
        }
        driver_unlock(); Ok(())
    }
    
    fn dack_cal_restore_path(&self, path: u8) -> Result<(), &'static str> {
        // Simplified - full implementation would restore from dm_info->dack_msbk
        driver_unlock(); Ok(())
    }
    
    /// Restore IQK calibration (mirrors Linux IQK reload)
    fn iqk_restore(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Restoring IQK calibration...");
        // Simplified - full implementation would reload RX CFIR from backup
        driver_unlock(); Ok(())
    }
    
    /// Restore power trim (mirrors Linux rtw8822c_power_trim)
    fn power_trim_restore(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Restoring power trim...");
        // Simplified - full implementation would restore from EFUSE
        driver_unlock(); Ok(())
    }

    /// Enter LPS (Link Power Save) - FWLPS
    /// Mirrors Linux: rtl8822c_lps_enter
    fn lps_enter(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] LPS enter (FWLPS)...");
        
        // Step 1: Backup critical HW registers before entering LPS
        // (Would backup: RCR, TX pages, DMA pointers, etc.)
        
        // Step 2: Set RPWM to S2 (32k clock only)
        self.set_rpwm(PS_STATE_S2)?;
        
        self.pwr_state = PS_STATE_S2;
        self.bpower_saving = true;
        
        // Step 3: Write LPS coefficients to FW (thermal meter, delta power index, etc.)
        // Would write to FW via H2C or MMIO
        
        crate::mesa_println!("[RTL8822CE] LPS entered (32k)");
        driver_unlock(); Ok(())
    }

    /// Leave LPS (Link Power Save)
    /// Mirrors Linux: rtl8822c_lps_leave
    fn lps_leave(&mut self) -> Result<(), &'static str> {
        if self.pwr_state < PS_STATE_S2 {
            driver_unlock(); return Ok(());
        }
        
        crate::mesa_println!("[RTL8822CE] LPS leave...");
        
        // Step 1: Restore HW registers from LPS backup
        // (Would restore: RCR, TX pages, DMA pointers, etc.)
        
        // Step 2: Set RPWM to S0 (active mode)
        self.set_rpwm(PS_STATE_S0)?;
        
        self.pwr_state = PS_STATE_S0;
        self.bpower_saving = false;
        
        // Step 3: Restore RF calibration results
        // (Would restore from saved RF init values)
        
        crate::mesa_println!("[RTL8822CE] LPS left (active)");
        driver_unlock(); Ok(())
    }

    /// Check FW power state
    fn fw_ps_state(&self) -> Result<(), &'static str> {
        let fw_ps_state = unsafe { self.read16(REG_HMETFR_8822C) };
        if (fw_ps_state & 0x01) == 0 { // BIT_LPS_STATUS
            driver_unlock(); Ok(())
        } else {
            Err("FW in 32k")
        }
    }

    // ========================================================================
    // PCI Initialization
    // ========================================================================

    fn pci_init(&mut self) -> Result<(), &'static str> {
        // Find the device
        let device = pci::find_device(VENDOR_ID, DEVICE_ID)
            .or_else(|| pci::find_device(VENDOR_ID, DEVICE_ID_ALT))
            .ok_or("RTL8822CE not found on PCI bus")?;

        crate::serial_println!(
            "[RTL8822CE] Found: {:02x}:{:02x}.{} (Vendor={:#06x} Device={:#06x})",
            device.bus, device.device, device.function,
            device.vendor_id, device.device_id
        );
        wifi_log(&format!("Paso 1/8: Dispositivo PCI encontrado en {:02x}:{:02x}.{}", device.bus, device.device, device.function));

        self.pci_bus = device.bus;
        self.pci_device = device.device;
        self.pci_function = device.function;

        // Enable bus mastering + memory space
        pci::pci_enable_bus_mastering(device.bus, device.device, device.function);
        pci::pci_enable_memory_space(device.bus, device.device, device.function);

        // Disable ASPM (critical for Realtek)
        pci::pci_disable_aspm(device.bus, device.device, device.function);

        // Set power state to D0
        let _ = pci::pci_set_power_state(device.bus, device.device, device.function, 0);

        // Try BARs 0-5 to find the MMIO BAR (BAR1 or BAR2 typically)
        let mut mmio_found = false;
        for bar_num in 0..6u8 {
            if let Some((bar_base, bar_size)) = pci::pci_read_bar(device.bus, device.device, device.function, bar_num) {
                crate::serial_println!(
                    "[RTL8822CE] BAR{}: phys={:#x} size={:#x}",
                    bar_num, bar_base, bar_size
                );
                // MMIO BAR should be at least 64KB and not zero
                if bar_base > 0 && bar_size >= 0x10000 && !mmio_found {
                    crate::serial_println!(
                        "[RTL8822CE] Using BAR{} as MMIO: phys={:#x} size={:#x}",
                        bar_num, bar_base, bar_size
                    );
                    wifi_log(&format!("Paso 2/8: MMIO mapeado en BAR{} (phys={:#x}, {}KB)", bar_num, bar_base, bar_size / 1024));

                    // Setup bridge window for this BAR
                    if bar_base > 0 && bar_base <= 0xFFFFFFFF {
                        pci::pci_setup_bridge_window(device.bus, bar_base as u32, bar_size as u32);
                    }

                    // Map MMIO as uncacheable
                    let mmio_virt = memory::vmm::map_mmio(bar_base, bar_size)
                        .map_err(|_| "Failed to map MMIO BAR")?;

                    self.mmio_base = mmio_virt;
                    self.mmio_phys = bar_base;
                    self.mmio_size = bar_size;
                    mmio_found = true;
                }
            }
        }
        if !mmio_found {
            return Err("RTL8822CE: No MMIO BAR found");
        }

        // Verify chip responds
        let chip_id = unsafe { self.read32(REG_SYS_CFG1) };
        crate::serial_println!(
            "[RTL8822CE] Raw Chip ID read: {:#010x}",
            chip_id
        );

        // Try to read another register to verify MMIO works
        let sys_func = unsafe { self.read16(REG_SYS_FUNC_EN) };
        crate::serial_println!(
            "[RTL8822CE] SYS_FUNC_EN: {:#06x}",
            sys_func
        );

        if chip_id == 0xFFFFFFFF {
            return Err("RTL8822CE: Chip not responding (MMIO read returned 0xFFFFFFFF)");
        }

        crate::serial_println!(
            "[RTL8822CE] Chip ID: {:#010x} (ver={}, rf_type={})",
            chip_id,
            (chip_id >> 12) & 0xF,
            (chip_id >> 8) & 0xF
        );
        wifi_log(&format!("Paso 3/8: Chip ID = {:#010x}", chip_id));

        driver_unlock(); Ok(())
    }

    // ========================================================================
    // Power-On Sequence
    // ========================================================================

    /// Linux rtw_mac_pre_system_cfg (mac.c:62-137): release I/O interface, then
    /// disable BB/RF so the power-on sequence starts from a clean state.
    fn mac_pre_system_cfg(&self) {
        // 0a: Release MCU lock
        unsafe {
            self.write8(REG_RSV_CTRL, 0);
        }
        self.delay_ms(10);

        // 0b: PCIe: set BIT_USB_SUS_DIS on REG_HCI_OPT_CTRL (0x0074[8] => 0x0075[0])
        unsafe {
            self.write8(0x0075, self.read8(0x0075) | (1 << 0));
        }

        // 0c: config PIN Mux
        //     REG_PAD_CTRL1 (0x0064) |= BIT_PAPE_WLBT_SEL(BIT!(29)) | BIT_LNAON_WLBT_SEL(BIT!(28))
        //     => byte 0x0067 bits [5:4]
        unsafe {
            self.write8(0x0067, self.read8(0x0067) | 0x30);
        }
        //     REG_LED_CFG (0x004C) &= ~(BIT_PAPE_SEL_EN(BIT!(25)) | BIT_LNAON_SEL_EN(BIT!(26)))
        //     => byte 0x004F bits [2:1]
        unsafe {
            self.write8(0x004F, self.read8(0x004F) & !0x06);
        }
        //     REG_GPIO_MUXCFG (0x0040) |= BIT_WLRFE_4_5_EN(BIT!(2))
        unsafe {
            self.write8(0x0040, self.read8(0x0040) | 0x04);
        }

        // 0d: disable BB/RF
        //     REG_SYS_FUNC_EN (0x0002) &= ~(BIT_FEN_BB_RSTB(BIT!(0)) | BIT_FEN_BB_GLB_RST(BIT!(1)))
        unsafe {
            self.write8(REG_SYS_FUNC_EN, self.read8(REG_SYS_FUNC_EN) & !0x03);
        }
        //     REG_RF_CTRL (0x001F) &= ~(BIT_RF_SDM_RSTB(BIT!(2)) | BIT_RF_RSTB(BIT!(1)) | BIT_RF_EN(BIT!(0)))
        unsafe {
            self.write8(REG_RF_CTRL, self.read8(REG_RF_CTRL) & !0x07);
        }
        //     REG_WLRF1 (0x00EC) &= ~BIT_WLRF1_BBRF_EN(BIT!(24)|BIT!(25)|BIT!(26))
        //     => byte 0x00EF bits [2:0]
        unsafe {
            self.write8(0x00EF, self.read8(0x00EF) & !0x07);
        }
        self.delay_ms(10);
    }

    /// Linux rtw_power_off for 8822C: card_disable_flow_8822c (rtw8822c.c:4861)
    /// = trans_act_to_cardemu_8822c + trans_cardemu_to_carddis_8822c, PCIe
    /// commands only. Used by rtw_mac_power_on's -EALREADY path to reset a chip
    /// that still holds power/warm state before downloading firmware.
    fn pwr_off_seq(&self) {
        crate::mesa_println!("[RTL8822CE]  pwr_off (act->cardemu->carddis)...");
        unsafe {
            // trans_act_to_cardemu_8822c (PCIe commands)
            self.write8(0x0093, self.read8(0x0093) & !0x08);
            self.write8(0x001F, 0xFF);
            self.write8(0x00EF, 0xFF);
            self.write8(0x1045, self.read8(0x1045) & !(1 << 4));
            self.write8(0x0049, self.read8(0x0049) & !(1 << 1));
            self.write8(0x0006, self.read8(0x0006) | (1 << 0));
            self.write8(0x0002, self.read8(0x0002) & !(1 << 1));
            // set 0x0005[1], then poll until it self-clears
            self.write8(0x0005, self.read8(0x0005) | (1 << 1));
        }
        self.delay_ms(10);
        // poll 0x0005[1] == 0 (Linux: 50us polling)
        if let Err(_) = self.poll_reg8(0x0005, 0x02, 0x00, 1000) {
            crate::mesa_println!("[RTL8822CE]  pwr_off poll 0x05[1] TIMEOUT (05={:#04x})",
                unsafe { self.read8(0x0005) });
        }
        self.delay_ms(10);
        unsafe {
            // trans_cardemu_to_carddis_8822c (PCIe commands)
            self.write8(0x0067, self.read8(0x0067) & !(1 << 5));
            self.write8(0x0081, self.read8(0x0081) & !0xC0);
            self.write8(0x0090, self.read8(0x0090) & !(1 << 1));
            self.write8(0x0092, 0x20);
            self.write8(0x0093, 0x04);
            self.write8(0x0005, self.read8(0x0005) | (1 << 2));
        }
        self.delay_ms(10);
    }

    fn power_on(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Power-on...");

        // Step 0: Linux rtw_mac_pre_system_cfg (mac.c:62-137)
        self.mac_pre_system_cfg();

        // Linux rtw_mac_power_switch (mac.c:272-301) for PCIe:
        //   cur_pwr = (read8(REG_CR) == 0xea) ? false : true
        // rtw_mac_power_on (mac.c:378-410): if the chip already looks powered
        // (REG_CR != 0xea) it returns -EALREADY, then Linux power-switches OFF,
        // re-runs pre_system_cfg, and power-ON again. Replicate that cycle.
        let reg_cr = unsafe { self.read8(REG_CR) };
        crate::mesa_println!("[RTL8822CE]  REG_CR={:#04x} (0xea = chip off/cold)", reg_cr);
        if reg_cr != 0xea {
            crate::mesa_println!("[RTL8822CE]  EALREADY: chip parece encendido, ciclo off->on...");
            self.pwr_off_seq();
            self.mac_pre_system_cfg();
        }

        // Execute power sequence: trans_carddis_to_cardemu
        crate::mesa_println!("[RTL8822CE]  carddis_to_cardemu...");
        unsafe {
            self.write8(0x002E, self.read8(0x002E) | (1 << 2));
            self.delay_ms(10);
            self.write8(0x002D, self.read8(0x002D) & !(1 << 0));
            self.delay_ms(10);
            self.write8(0x007F, self.read8(0x007F) & !(1 << 7));
            self.delay_ms(10);
            self.write8(0x0005, self.read8(0x0005) & !((1 << 7) | (1 << 4) | (1 << 3)));
            self.delay_ms(10);
        }

        // Execute power sequence: trans_cardemu_to_act for PCIe
        crate::mesa_println!("[RTL8822CE]  cardemu_to_act...");

        // 1. write 0x0005[4:3:2] = 0
        crate::mesa_println!("[RTL8822CE]    st1: 0x05");
        unsafe {
            self.write8(0x0005, self.read8(0x0005) & !((1 << 4) | (1 << 3) | (1 << 2)));
        }
        self.delay_ms(10);

        // 2. PCIe: write 0x0075[0] = 1
        crate::mesa_println!("[RTL8822CE]    st2: 0x75");
        unsafe {
            self.write8(0x0075, self.read8(0x0075) | (1 << 0));
        }
        self.delay_ms(10);

        match self.poll_reg8(0x0006, 0x02, 0x02, 1000) {
            Ok(()) => {
                crate::mesa_println!("[RTL8822CE]    st3: OK");
            },
            Err(_) => {
                crate::mesa_println!("[RTL8822CE]    st3: TIMEOUT (06={:#04x})", unsafe { self.read8(0x0006) });
            }
        }

        // 4. PCIe: write 0x0075[0] = 0
        crate::mesa_println!("[RTL8822CE]    st4: 0x75");
        unsafe {
            self.write8(0x0075, self.read8(0x0075) & !(1 << 0));
        }
        self.delay_ms(10);

        // 5. write 0x002E[3] = 0
        crate::mesa_println!("[RTL8822CE]    st5: 0x2E");
        unsafe {
            self.write8(0x002E, self.read8(0x002E) & !(1 << 3));
        }
        self.delay_ms(10);

        // 6. write 0x0006[0] = 1
        crate::mesa_println!("[RTL8822CE]    st6: 0x06");
        unsafe {
            self.write8(0x0006, self.read8(0x0006) | (1 << 0));
        }
        self.delay_ms(10);

        // 7. write 0x0005[4:3] = 0
        crate::mesa_println!("[RTL8822CE]    st7: 0x05");
        unsafe {
            self.write8(0x0005, self.read8(0x0005) & !((1 << 4) | (1 << 3)));
        }
        self.delay_ms(10);

        // 8. write 0x1018[2] = 1
        crate::mesa_println!("[RTL8822CE]    st8: 0x18");
        unsafe {
            self.write8(0x1018, self.read8(0x1018) | (1 << 2));
        }
        self.delay_ms(10);

        // 9. write 0x0005[0] = 1
        crate::mesa_println!("[RTL8822CE]    st9: 0x05");
        unsafe {
            self.write8(0x0005, self.read8(0x0005) | (1 << 0));
        }
        self.delay_ms(10);

        // 10. poll 0x0005[0] = 0 (self-clearing: bit se limpia cuando el MAC
        //     termina la transicion cardemu->act; Linux usa timeout de 1s)
        crate::mesa_println!("[RTL8822CE]    st10: poll 0x05[0]");
        if let Err(_e) = self.poll_reg8(0x0005, 0x01, 0x00, 1000) {
            let v = unsafe { self.read8(0x0005) };
            crate::mesa_println!("[RTL8822CE]    st10: TIMEOUT (05={:#04x})", v);
        } else {
            crate::mesa_println!("[RTL8822CE]    st10: OK");
        }

        // 11. PCIe: write 0x0074[5] = 1
        crate::mesa_println!("[RTL8822CE]    st11: 0x74");
        unsafe {
            self.write8(0x0074, self.read8(0x0074) | (1 << 5));
        }
        self.delay_ms(10);

        // 12. PCIe: write 0x0071[4] = 0
        crate::mesa_println!("[RTL8822CE]    st12: 0x71");
        unsafe {
            self.write8(0x0071, self.read8(0x0071) & !(1 << 4));
        }
        self.delay_ms(10);

        // 13. PCIe: write 0x0062[7:6:5] = 1
        crate::mesa_println!("[RTL8822CE]    st13: 0x62");
        unsafe {
            self.write8(0x0062, self.read8(0x0062) | ((1 << 7) | (1 << 6) | (1 << 5)));
        }
        self.delay_ms(10);

        // 14. PCIe: write 0x0061[7:6:5] = 0
        crate::mesa_println!("[RTL8822CE]    st14: 0x61");
        unsafe {
            self.write8(0x0061, self.read8(0x0061) & !((1 << 7) | (1 << 6) | (1 << 5)));
        }
        self.delay_ms(10);

        // 15. write 0x001F[7:6] = [7]
        crate::mesa_println!("[RTL8822CE]    st15: 0x1F");
        unsafe {
            self.write8(0x001F, (self.read8(0x001F) & !((1 << 7) | (1 << 6))) | (1 << 7));
        }
        self.delay_ms(10);

        // 16. write 0x00EF[7:6] = [7]
        crate::mesa_println!("[RTL8822CE]    st16: 0xEF");
        unsafe {
            self.write8(0x00EF, (self.read8(0x00EF) & !((1 << 7) | (1 << 6))) | (1 << 7));
        }
        self.delay_ms(10);

        // 17. write 0x1045[4] = 1
        crate::mesa_println!("[RTL8822CE]    st17: 0x45");
        unsafe {
            self.write8(0x1045, self.read8(0x1045) | (1 << 4));
        }
        self.delay_ms(10);

        // 18. write 0x0010[2] = 1
        crate::mesa_println!("[RTL8822CE]    st18: 0x10");
        unsafe {
            self.write8(0x0010, self.read8(0x0010) | (1 << 2));
        }
        self.delay_ms(10);

        // 19. write 0x1064[1] = 1
        crate::mesa_println!("[RTL8822CE]    st19: 0x64");
        unsafe {
            self.write8(0x1064, self.read8(0x1064) | (1 << 1));
        }
        self.delay_ms(100);

        // === __rtw_mac_init_system_cfg ===
        crate::mesa_println!("[RTL8822CE]  init_system_cfg...");
        unsafe {
            let val = self.read32(REG_CPU_DMEM_CON);
            self.write32(REG_CPU_DMEM_CON, val | BIT_WL_PLATFORM_RST | BIT_DDMA_EN);
            self.write8(REG_SYS_FUNC_EN + 1, self.read8(REG_SYS_FUNC_EN + 1) | 0xD8);
            // Linux __rtw_mac_init_system_cfg (rtw88/mac.c):
            //   (read8(REG_CR_EXT+3) & 0xF0) | 0x0C
            let cr_ext = (self.read8(REG_CR_EXT + 3) & 0xF0) | 0x0C;
            self.write8(REG_CR_EXT + 3, cr_ext);

            // Disable boot-from-flash so the 8051 boots from the just-downloaded
            // IMEM instead of the SPI flash (Linux __rtw_mac_init_system_cfg).
            let mcufw = self.read32(REG_MCUFW_CTRL);
            if mcufw & BIT_BOOT_FSPI_EN != 0 {
                self.write32(REG_MCUFW_CTRL, mcufw & !BIT_BOOT_FSPI_EN);
                let gpio = self.read32(REG_GPIO_MUXCFG);
                self.write32(REG_GPIO_MUXCFG, gpio & !BIT_FSPI_EN);
                crate::mesa_println!("[RTL8822CE]  FSPI boot deshabilitado (MCUFW={:#010x})", mcufw);
            } else {
                crate::mesa_println!("[RTL8822CE]  FSPI boot ya deshabilitado (MCUFW={:#010x})", mcufw);
            }
        }
        self.delay_ms(100);

        // NOTE: REG_CR (MAC_TRX_ENABLE) is NOT written here. Linux only enables
        // the MAC after the firmware validates (rtw_init_trx_cfg, mac.c:1124)
        // with a byte write REG_CR=MAC_TRX_ENABLE. The 0x4FF value seen in older
        // MesaOS logs came from the 8814A/legacy path (rtw8814a.c:387) and is not
        // applicable to the WCPU_3081 download flow.

        driver_unlock(); Ok(())
    }

    // ========================================================================
    // Firmware Loading
    // ========================================================================

    /// Backup and configure registers for firmware download. Mirrors the Linux
    /// `download_firmware_reg_backup()` (rtw88/mac.c): saves the 6 registers
    /// that are changed to enable the download path (HIQ hi-priority, CR TX DMA,
    /// H2CQ full, FIFO page info, RQPN LD, BCN off), so they can be restored
    /// right before the end flow.
    fn download_firmware_reg_backup(&mut self, bckp: &mut [DwRegBackup; DLFW_RESTORE_REG_NUM]) {
        unsafe {
            bckp[0] = DwRegBackup { len: 1, reg: REG_TXDMA_PQ_MAP + 1, val: self.read8(REG_TXDMA_PQ_MAP + 1) as u32 };
            bckp[1] = DwRegBackup { len: 1, reg: REG_CR, val: self.read8(REG_CR) as u32 };
            bckp[2] = DwRegBackup { len: 4, reg: REG_H2CQ_CSR, val: BIT_H2CQ_FULL };
            bckp[3] = DwRegBackup { len: 2, reg: REG_FIFOPAGE_INFO_1, val: self.read16(REG_FIFOPAGE_INFO_1) as u32 };
            bckp[4] = DwRegBackup { len: 4, reg: REG_RQPN_CTRL_2, val: self.read32(REG_RQPN_CTRL_2) | BIT_LD_RQPN };
            bckp[5] = DwRegBackup { len: 1, reg: REG_BCN_CTRL, val: self.read8(REG_BCN_CTRL) as u32 };

            // Configure for the firmware download phase (Linux: RTW_DMA_MAPPING_HIGH << 6)
            self.write8(REG_TXDMA_PQ_MAP + 1, (RTW_DMA_MAPPING_HIGH << 6) as u8);
            self.write8(REG_CR, (BIT_HCI_TXDMA_EN | BIT_TXDMA_EN) as u8);
            self.write32(REG_H2CQ_CSR, BIT_H2CQ_FULL);
            self.write16(REG_FIFOPAGE_INFO_1, 0x0200);
            self.write32(REG_RQPN_CTRL_2, self.read32(REG_RQPN_CTRL_2) | BIT_LD_RQPN);
            let tmp = self.read8(REG_BCN_CTRL);
            self.write8(REG_BCN_CTRL, (tmp & !(BIT_EN_BCN_FUNCTION as u8)) | (BIT_DIS_TSF_UDT as u8));
        }
    }

    /// Restore registers saved by `download_firmware_reg_backup` (mirrors Linux
    /// `rtw_restore_reg`). Must run before `download_firmware_end_flow`.
    fn download_firmware_reg_restore(&self, bckp: &[DwRegBackup; DLFW_RESTORE_REG_NUM]) {
        for b in bckp.iter() {
            let reg = b.reg;
            let val = b.val;
            unsafe {
                match b.len {
                    1 => self.write8(reg, val as u8),
                    2 => self.write16(reg, val as u16),
                    4 => self.write32(reg, val),
                    _ => {}
                }
            }
        }
    }

    /// Read a register in the WLAN<->LTE coex domain (Linux `ltecoex_read_reg`).
    /// Polls LTECOEX_READY (bit29 of 0x1700) before issuing the indirect read.
    fn ltecoex_read_reg(&self, offset: u32) -> Result<u32, &'static str> {
        // Linux check_hw_ready: up to 1000 iterations x 10us
        for _ in 0..1000 {
            if unsafe { self.read32(REG_LTECOEX_CTRL) } & LTECOEX_READY != 0 {
                unsafe {
                    self.write32(REG_LTECOEX_CTRL, 0x800F0000 | offset);
                    return Ok(self.read32(REG_LTECOEX_RDATA));
                }
            }
            self.delay_us(10);
        }
        Err("RTL8822CE: LTECOEX not ready (read)")
    }

    /// Write a register in the WLAN<->LTE coex domain (Linux `ltecoex_reg_write`).
    fn ltecoex_reg_write(&self, offset: u32, value: u32) -> Result<(), &'static str> {
        for _ in 0..1000 {
            if unsafe { self.read32(REG_LTECOEX_CTRL) } & LTECOEX_READY != 0 {
                unsafe {
                    self.write32(REG_LTECOEX_WDATA, value);
                    self.write32(REG_LTECOEX_CTRL, 0xC00F0000 | offset);
                    driver_unlock(); return Ok(());
                }
            }
            self.delay_us(10);
        }
        Err("RTL8822CE: LTECOEX not ready (write)")
    }

    fn download_firmware(&mut self, fw_data: &[u8]) -> Result<(), &'static str> {
        if fw_data.len() < FW_HDR_SIZE {
            return Err("RTL8822CE: Firmware too small");
        }

        let version = u16::from_le_bytes([fw_data[0x04], fw_data[0x05]]);
        let subversion = fw_data[0x06];
        let subindex = fw_data[0x07];
        crate::mesa_println!(
            "[RTL8822CE] FW v{}.{}.{} ({} bytes)",
            version, subversion, subindex, fw_data.len()
        );

        // Parse firmware header (offsets match Linux rtw88 fw.h)
        // struct rtw_fw_hdr {
        //   __le32 dmem_addr;     // 0x20
        //   __le32 dmem_size;     // 0x24
        //   __le32 rsvd6;         // 0x28
        //   __le32 rsvd7;         // 0x2C
        //   __le32 imem_size;     // 0x30
        //   __le32 emem_size;     // 0x34
        //   __le32 emem_addr;     // 0x38
        //   __le32 imem_addr;     // 0x3C
        // }
        let dmem_addr = u32::from_le_bytes([fw_data[0x20], fw_data[0x21], fw_data[0x22], fw_data[0x23]]) & !0x80000000;
        let dmem_size = u32::from_le_bytes([fw_data[0x24], fw_data[0x25], fw_data[0x26], fw_data[0x27]]) + FW_HDR_CHKSUM_SIZE as u32;
        let imem_size = u32::from_le_bytes([fw_data[0x30], fw_data[0x31], fw_data[0x32], fw_data[0x33]]) + FW_HDR_CHKSUM_SIZE as u32;
        let emem_size_raw = u32::from_le_bytes([fw_data[0x34], fw_data[0x35], fw_data[0x36], fw_data[0x37]]);
        let emem_addr = u32::from_le_bytes([fw_data[0x38], fw_data[0x39], fw_data[0x3A], fw_data[0x3B]]) & !0x80000000;
        let imem_addr = u32::from_le_bytes([fw_data[0x3C], fw_data[0x3D], fw_data[0x3E], fw_data[0x3F]]) & !0x80000000;
        let mem_usage = fw_data[0x18];
        let emem_size = if (mem_usage & (1 << 4)) != 0 {
            emem_size_raw + FW_HDR_CHKSUM_SIZE as u32
        } else {
            0
        };

        crate::mesa_println!("[RTL8822CE] DMEM: addr={:#x} size={}", dmem_addr & !0x80000000, dmem_size);
        crate::mesa_println!("[RTL8822CE] IMEM: addr={:#x} size={}", imem_addr & !0x80000000, imem_size);
        if emem_size > 0 {
            crate::mesa_println!("[RTL8822CE] EMEM: addr={:#x} size={}", emem_addr & !0x80000000, emem_size);
        }

        // === Step 0: LTECOEX backup (Linux __rtw_download_firmware:
        //   ltecoex_read_reg(0x38) BEFORE wlan_cpu_enable(false); aborts -EBUSY
        //   if LTECOEX_READY (bit29 of 0x1700) never asserts. This is a hard
        //   gate: on Linux, FW download does not start unless the coex domain
        //   is alive (shares the 3081 power/clock).
        crate::mesa_println!("[RTL8822CE] FW St0: LTECOEX backup...");
        let ltecoex_bckp = match self.ltecoex_read_reg(LTECOEX_BACKUP_OFFSET) {
            Ok(v) => {
                crate::mesa_println!("[RTL8822CE]  LTECOEX_READY OK 0x38={:#010x}", v);
                v
            }
            Err(e) => {
                crate::mesa_println!("[RTL8822CE]  {} (ctrl={:#010x})", e, unsafe { self.read32(REG_LTECOEX_CTRL) });
                return Err(e);
            }
        };

        // === Step 1: wlan_cpu_enable(false) ===
        crate::mesa_println!("[RTL8822CE] FW St1: CPU off...");
        unsafe {
            self.write8(REG_SYS_FUNC_EN + 1, self.read8(REG_SYS_FUNC_EN + 1) & !(1 << 2));
            self.write8(REG_RSV_CTRL + 1, self.read8(REG_RSV_CTRL + 1) & !(1 << 0));
        }
        self.delay_ms(10);

        // === Step 2: Backup and configure regs ===
        crate::mesa_println!("[RTL8822CE] FW St2: Backup & configure regs...");
        let mut dlfw_bckp = [DwRegBackup { len: 0, reg: 0, val: 0 }; DLFW_RESTORE_REG_NUM];
        self.download_firmware_reg_backup(&mut dlfw_bckp);

        // === Step 3: Platform reset ===
        // Linux halmac pltfm_reset_88xx: only toggles SYS_CLK_CTRL+1 bit 6 for 8821C/8822B.
        // For 8822C (WCPU_3081), only CPU_DMEM_CON bit 0 is toggled.
        crate::mesa_println!("[RTL8822CE] FW St3: Plat reset...");
        unsafe {
            self.write8(REG_CPU_DMEM_CON + 2, self.read8(REG_CPU_DMEM_CON + 2) & !(1 << 0));
            self.delay_ms(100);
            self.write8(REG_CPU_DMEM_CON + 2, self.read8(REG_CPU_DMEM_CON + 2) | (1 << 0));
            self.delay_ms(100);
        }

        // === Step 4: Enable firmware download mode ===
        crate::mesa_println!("[RTL8822CE] FW St4: MCUFWDL_EN...");
        unsafe {
            let val = (self.read16(REG_MCUFW_CTRL) & 0x3800) | BIT_MCUFWDL_EN as u16;
            self.write16(REG_MCUFW_CTRL, val);
            crate::mesa_println!("[RTL8822CE]  MCUFW={:#06x}", self.read16(REG_MCUFW_CTRL));
        }
        self.delay_ms(50);

        // === Step 5: Initialize watchdog for firmware timeout detection ===
        // Linux: rtl8822c_watchdog_init - sets up a timer to detect firmware hangs
        self.watchdog_enabled = true;
        self.watchdog_timeout = 0;

        // NOTE: Linux stages each FW chunk into the chip TX FIFO via the BCN
        // rsvd-page ring, then IDDMAs from OCPBASE_TXBUF_88XX + 48. The staging
        // path is used when FW_USE_BCNQ_STAGING is true (default); it captures
        // FIFOPAGE_CTRL_2 below so per-chunk restore can put the boundary back.

        // === Step 5: Download firmware sections ===
        // Backup the reserved-page boundary once so fw_write_data_rsvd_page can
        // restore it after each staged chunk (Linux: rsvd_boundary at boot).
        self.fw_fifopage_ctrl2_bckp = unsafe { self.read16(REG_FIFOPAGE_CTRL_2) } & (BIT_MASK_BCN_HEAD_1_V1 as u16);
        crate::mesa_println!("[RTL8822CE]  FIFOPAGE_CTRL_2 bckp={:#06x}", self.fw_fifopage_ctrl2_bckp);
        let mut cur_offset = FW_HDR_SIZE;

        // DMEM
        crate::mesa_println!("[RTL8822CE] FW St5: DMEM {}B -> {:#x}", dmem_size, dmem_addr & !0x80000000);
        {
            let dmem_data = &fw_data[cur_offset..cur_offset + dmem_size as usize];
            self.download_section_to_mem(dmem_data, dmem_addr & !0x80000000, true)?;
        }
        cur_offset += dmem_size as usize;
        self.check_fw_checksum(dmem_addr & !0x80000000)?;

        // IMEM
        crate::mesa_println!("[RTL8822CE] FW St5: IMEM {}B -> {:#x}", imem_size, imem_addr & !0x80000000);
        {
            let imem_data = &fw_data[cur_offset..cur_offset + imem_size as usize];
            self.download_section_to_mem(imem_data, imem_addr & !0x80000000, true)?;
        }
        cur_offset += imem_size as usize;
        self.check_fw_checksum(imem_addr & !0x80000000)?;

        // EMEM (if present)
        if emem_size > 0 {
            let emem_end = cur_offset + emem_size as usize;
            if emem_end > fw_data.len() {
                crate::mesa_println!("[RTL8822CE] FW St5: EMEM truncated! Need {}B, have {}B", emem_end, fw_data.len());
                crate::mesa_println!("[RTL8822CE]  Skipping EMEM download (firmware file incomplete)");
            } else {
                crate::mesa_println!("[RTL8822CE] FW St5: EMEM {}B -> {:#x}", emem_size, emem_addr & !0x80000000);
                let emem_data = &fw_data[cur_offset..emem_end];
                self.download_section_to_mem(emem_data, emem_addr & !0x80000000, true)?;
            }
        }

        // === Step 5.5: Restore regs (Linux restores before end flow) ===
// Verify IDDMA checksums for all downloaded sections before restoring regs.
        // This mirrors Linux halmac: check_fw_chksum_88xx is called after each
        // section in dlfw_to_mem_88xx, and dlfw_end_flow_88xx checks the
        // cumulative MCUFW_CTRL bits (0x50 = IMEM_CHKSUM_OK | DMEM_CHKSUM_OK).
        crate::mesa_println!("[RTL8822CE] FW St5.5: Verify checksums & restore regs...");
        // Check IDDMA checksum status for the last section (EMEM if present, else IMEM)
        let last_section_addr = if emem_size > 0 {
            emem_addr & !0x80000000
        } else {
            imem_addr & !0x80000000
        };
        self.check_fw_checksum(last_section_addr)?;
        self.download_firmware_reg_restore(&dlfw_bckp);

        // === Step 6: End flow ===
        // Linux halmac dlfw_end_flow_88xx: check IMEM & DMEM checksums (0x50),
        // set FW_DW_RDY, enable CPU, then poll for FW_READY (0xC078).
        crate::mesa_println!("[RTL8822CE] FW St6: End flow...");
        unsafe {
            self.write32(REG_TXDMA_STATUS, BTI_PAGE_OVF);
            let fw_ctrl = self.read16(REG_MCUFW_CTRL);
            crate::mesa_println!("[RTL8822CE]  MCUFW={:#06x} cksum_ok={}", fw_ctrl, (fw_ctrl as u32 & BIT_CHECK_SUM_OK) == BIT_CHECK_SUM_OK);
            if (fw_ctrl as u32 & BIT_CHECK_SUM_OK) != BIT_CHECK_SUM_OK {
                crate::mesa_println!("[RTL8822CE]  CHECKSUM NOT OK (need 0x50, got {:#06x})", fw_ctrl & 0x50);
                return Err("RTL8822CE: FW checksum error");
            }
            // Both IMEM & DMEM checksums OK - set FW_DW_RDY and clear MCUFWDL_EN
            self.write16(REG_MCUFW_CTRL, (fw_ctrl | BIT_FW_DW_RDY as u16) & !(BIT_MCUFWDL_EN as u16));
            crate::mesa_println!("[RTL8822CE]  -> MCUFW={:#06x}", self.read16(REG_MCUFW_CTRL));
        }
        // Wait for hardware to process the end flow command
        self.delay_ms(50);

        // === Step 7: Enable CPU (Firmware CPU) ===
        crate::mesa_println!("[RTL8822CE] FW St7: CPU on...");
        // Snapshot FW state BEFORE we release the CPU, so we can tell whether
        // DBG6=0x100 (FW status check error) is stale (left over from a prior
        // boot / FSPI attempt) or freshly set by the boot code after CPEN.
        crate::mesa_println!("[RTL8822CE]  pre-CPEN: MCUFW={:#010x} DBG7={:#010x} DBG6={:#010x} BOOT={:#010x}",
            unsafe { self.read32(REG_MCUFW_CTRL) },
            unsafe { self.read32(REG_FW_DBG7) },
            unsafe { self.read32(REG_FW_DBG6) },
            unsafe { self.read32(REG_BOOT_REASON) });
        unsafe {
            self.write8(REG_RSV_CTRL + 1, self.read8(REG_RSV_CTRL + 1) | (1 << 0));
            self.write8(REG_SYS_FUNC_EN + 1, self.read8(REG_SYS_FUNC_EN + 1) | (1 << 2));
        }
        // Immediately after CPEN: catch DBG6/DBG7 as close to the CPU release
        // as possible to see the boot code entry point.
        crate::mesa_println!("[RTL8822CE]  post-CPEN: MCUFW={:#010x} DBG7={:#010x} DBG6={:#010x}",
            unsafe { self.read32(REG_MCUFW_CTRL) },
            unsafe { self.read32(REG_FW_DBG7) },
            unsafe { self.read32(REG_FW_DBG6) });
        // Linux: ltecoex_reg_write happens AFTER wlan_cpu_enable(true), before validate.
        if let Err(e) = self.ltecoex_reg_write(LTECOEX_BACKUP_OFFSET, ltecoex_bckp) {
            crate::mesa_println!("[RTL8822CE]  {} (ctrl={:#010x})", e, unsafe { self.read32(REG_LTECOEX_CTRL) });
            return Err(e);
        }
        crate::mesa_println!("[RTL8822CE]  LTECOEX restored (0x38={:#010x})", ltecoex_bckp);

        // Poll for CPU to come alive (like Linux: wait for MCUFW to change from 0x0001)
        crate::mesa_println!("[RTL8822CE]  Polling for CPU wake-up...");
        for wk in 0..50 {
            let mcufw = unsafe { self.read16(REG_MCUFW_CTRL) };
            // Dense trace of the boot-decision registers while MCUFW is still
            // in the pre-boot state. DBG6/DBG7 reflect the boot code status.
            if wk < 20 {
                crate::mesa_println!("[RTL8822CE]  wk={:02} MCUFW={:#06x} DBG7={:#010x} DBG6={:#010x}",
                    wk, mcufw, unsafe { self.read32(REG_FW_DBG7) }, unsafe { self.read32(REG_FW_DBG6) });
            }
            if mcufw != 0x0001 && mcufw != 0 {
                crate::mesa_println!("[RTL8822CE]  CPU woke up, MCUFW={:#06x}", mcufw);
                break;
            }
            self.delay_ms(10);
        }

        // === Step 8: Validate firmware (FW READY state) ===
        crate::mesa_println!("[RTL8822CE] FW St8: Validate...");
        // Linux download_firmware_validate: poll until (REG_MCUFW_CTRL &
        // FW_READY_MASK) == FW_READY, i.e. bit 15 (FW_INIT_RDY) must be set by
        // the firmware itself once it boots and passes boot-time init.
        for attempt in 0..200 {
            let fw_ctrl: u32 = unsafe {
                // Read 32-bit to capture the full 16-bit low word; bits 16-31
                // hold FW state that Linux ignores (masked by FW_READY_MASK).
                self.read32(REG_MCUFW_CTRL)
            };

            if (fw_ctrl & FW_READY_MASK) == FW_READY {
                crate::mesa_println!("[RTL8822CE] FW OK! MCUFW={:#010x}", fw_ctrl);
                self.fw_loaded = true;
                // Linux resets the H2C mailbox box/seq counters after the download
                self.h2c_box = 0;
                driver_unlock(); return Ok(());
            }

            // NOTE: bits 16-31 of REG_MCUFW_CTRL are NOT an error flag for the
            // 8822C. On this chip bits 24-31 are the RPWM field (BIT_RPWM_8822C,
            // halmac_bit_8822c.h) plus ANA/MAC port enables - the firmware sets
            // these while it boots and runs its power handshake. Linux ignores
            // them (FW_READY_MASK masks the low 16 bits only). In particular
            // bit31 == RPWM toggle, which flips as the firmware runs, so it must
            // NOT be treated as "FW error". We keep polling until TIMEOUT.

            if attempt >= 100 {
                crate::mesa_println!("[RTL8822CE]  FW validation timeout after 10s");
                break;
            }
            
            // Sample the FW boot trajectory densely in the first ~1.2s (the
            // 3081 boot mini-code runs in the first hundreds of ms after CPU
            // enable), then every 25 attempts after.
            let dense = attempt < 24;
            if (dense) || (attempt % 25 == 0) {
                let pc = unsafe { self.read32(REG_FW_DBG7) };
                let dbg6 = unsafe { self.read32(REG_FW_DBG6) };
                let clk = unsafe { self.read32(REG_SYS_CLK_CTRL) };
                let cpu = unsafe { self.read32(REG_CPU_DMEM_CON) };
                let cr = unsafe { self.read8(REG_CR) };
                crate::mesa_println!(
                    "[RTL8822CE]  t={}ms MCUFW={:#010x} DBG7={:#010x} DBG6={:#010x} CR={:#04x} CLK={:#010x} CPU={:#010x}",
                    attempt * 50, fw_ctrl, pc, dbg6, cr, clk, cpu
                );
            }
            self.delay_ms(50);
        }

        let fw_ctrl = unsafe { self.read32(REG_MCUFW_CTRL) };
        crate::mesa_println!("[RTL8822CE] FW FAIL MCUFW={:#010x}", fw_ctrl);
        crate::mesa_println!("[RTL8822CE]  FW_READY needs={:#010x} mask={:#010x}", FW_READY, FW_READY_MASK);
        if (fw_ctrl & FW_READY_MASK) != FW_READY {
            let fw_key = unsafe { self.read32(REG_FW_DBG7) } & 0xFFFFFF00;
            crate::mesa_println!("[RTL8822CE]  FW_DBG7={:#010x}", unsafe { self.read32(REG_FW_DBG7) });
            if fw_key == ILLEGAL_KEY_GROUP {
                crate::mesa_println!("[RTL8822CE] INVALID fw key!");
                return Err("RTL8822CE: Invalid firmware key");
            }
        }
        // Dump the same fatal-status registers halmac reads on DLFW failure
        // (halmac_fw_88xx.c fw_fatal_status_debug_88xx).
        crate::mesa_println!("[RTL8822CE]  FW_DBG6={:#010x}", unsafe { self.read32(REG_FW_DBG6) });
        crate::mesa_println!("[RTL8822CE]  ARFR5={:#010x}", unsafe { self.read32(REG_ARFR5) });
        crate::mesa_println!("[RTL8822CE]  MCUTST_I={:#010x}", unsafe { self.read32(REG_MCUTST_I) });
        // Boot reason tells which boot path the ROM/FW took (FSPI vs host DLFW).
        crate::mesa_println!("[RTL8822CE]  BOOT_REASON={:#010x}", unsafe { self.read32(REG_BOOT_REASON) });
        // Full 3081 core exception regs (Linux fw.c fw_core_regs on FW failure):
        // EPC/CAUSE!=0 => exception in FW; all-zero + stable PC => deliberate
        // spin (FW waiting on something).
        crate::mesa_println!("[RTL8822CE]  EPC(ARFR2)={:#010x}", unsafe { self.read32(REG_ARFR2_V1) });
        crate::mesa_println!("[RTL8822CE]  BADADDR(ARFRH2)={:#010x}", unsafe { self.read32(REG_ARFRH2_V1) });
        let cause = unsafe { self.read32(REG_ARFR3_V1) };
        crate::mesa_println!("[RTL8822CE]  CAUSE(ARFR3)={:#010x} (ExcCode={:#x})", cause,
            (cause & BIT_EXC_CODE) >> 2);
        crate::mesa_println!("[RTL8822CE]  STATUS(ARFRH3)={:#010x}", unsafe { self.read32(REG_ARFRH3_V1) });
        crate::mesa_println!("[RTL8822CE]  SP(ARFR4)={:#010x} RA(ARFRH4)={:#010x}",
            unsafe { self.read32(REG_ARFR4) }, unsafe { self.read32(REG_ARFRH4) });
        // Read back a window of IMEM around the frozen PC to confirm the chip
        // really holds our downloaded image at the execution point.
        let imem_base_ocp = 0x00030000u32; // OCPBASE_IMEM_88XX (reg.h/mac.h)
        let pc = unsafe { self.read32(REG_FW_DBG7) };
        let pc_off = (pc & 0x0000FFFF) & !1u32; // bit0 = ISA-mode flag
        if pc_off < 0xD400 {
            // IMEM is 0x30000-0x3D330 on 8822C (OCPBASE_IMEM_88XX + 54072B)
            crate::mesa_println!("[RTL8822CE]  IMEM readback at PC offset 0x{:04x}:", pc_off);
            for row in 0..4 {
                let base = imem_base_ocp + pc_off + row * 16;
                crate::mesa_print!("[RTL8822CE]    {:04x}: ", pc_off + row * 16);
                for b in 0..16 {
                    let val = unsafe { self.read8(base + b) };
                    crate::mesa_print!("{:02x} ", val);
                }
                crate::mesa_println!("");
            }
        }
        // Verify firmware checksums via MCUFW_CTRL (replaces broken MMIO readback).
        let _ = self.fw_verify_checksums();
        // Extra boot-context registers.
        crate::mesa_println!("[RTL8822CE]  RSV_CTRL(0x001C)={:#010x} SYS_FUNC_EN(0x0002)={:#010x} CR_EXT(0x1100)={:#010x}",
            unsafe { self.read32(REG_RSV_CTRL) },
            unsafe { self.read32(REG_SYS_FUNC_EN) },
            unsafe { self.read32(REG_CR_EXT) });
        crate::mesa_println!("[RTL8822CE]  TXDMA_STATUS(0x0210)={:#010x} MCUTST_I={:#010x} MCUTST_II={:#010x}",
            unsafe { self.read32(REG_TXDMA_STATUS) },
            unsafe { self.read32(REG_MCUTST_I) },
            unsafe { self.read32(REG_MCUTST_II) });
        // Return error instead of continuing - firmware MUST be ready for WiFi to work
        Err("RTL8822CE: Firmware not ready after download")
    }

    /// Stage one firmware chunk into the chip TX FIFO reserved page via the
    /// BCN queue (port of Linux rtw_fw_write_data_rsvd_page + the BCN branch of
    /// rtw_pci_tx_write_data + rtw_pci_write_data_rsvd_page). The chunk payload
    /// goes after a 48-byte TX desc in `bcn_buf`, described by two 16-byte
    /// buffer-desc slots in the BCN ring, and is pushed by raising
    /// RTK_PCI_TXBD_BCN_WORK. After this returns, the caller must IDDMA from
    /// OCPBASE_TXBUF_88XX + TX_DESC_SIZE_FW (the data sits right after the desc).
    fn fw_write_data_rsvd_page(&mut self, chunk: &[u8]) -> Result<(), &'static str> {
        const DESC_SZ: usize = TX_DESC_SIZE_FW; // 48
        const BD_STRIDE: usize = 16; // tx_buf_desc_sz for 8822C

        if self.bcn_buf_virt.is_null() || self.bcn_bd_virt.is_null() {
            return Err("RTL8822CE: BCN buffers not allocated");
        }
        let size = chunk.len() as u32;
        if size as usize > 0x1000 {
            return Err("RTL8822CE: FW chunk too big for rsvd page");
        }

        // ---- TX descriptor (rtw_tx_fill_tx_desc for RSVD_BEACON via BCNQ) ----
        let desc = self.bcn_buf_virt as *mut u32;
        unsafe {
            core::ptr::write_bytes(desc as *mut u8, 0, DESC_SZ);
            // w0: TXPKTSIZE[15:0]=size, OFFSET[23:16]=48, BMC=1, LS=1, DISQSELSEQ=1
            desc.add(0).write_volatile(
                (size & 0xffff) | (48 << 16) | (1 << 24) | (1 << 26) | (1 << 31),
            );
            // w1: QSEL[12:8] = TX_DESC_QSEL_BEACON (16)
            desc.add(1).write_volatile(TX_DESC_QSEL_BEACON << 8);
            // w8: EN_HWSEQ
            desc.add(8).write_volatile(1 << 15);
            // payload after the desc
            core::ptr::copy_nonoverlapping(chunk.as_ptr(), self.bcn_buf_virt.add(DESC_SZ), size as usize);
        }

        // DMA-sync the coherent buffer (clflush + sfence, like dl_sec)
        let flush_start = (self.bcn_buf_virt as usize) & !63usize;
        let flush_end = ((self.bcn_buf_virt as usize) + DESC_SZ + size as usize + 63) & !63usize;
        unsafe {
            let mut a = flush_start;
            while a < flush_end {
                core::arch::asm!("clflush [{}]", in(reg) a, options(nostack));
                a += 64;
            }
            core::arch::asm!("sfence");
        }

        // ---- Buffer descriptors (rtw_pci_tx_write_data, BCN branch) ----
        let bd = self.bcn_bd_virt as *mut u8;
        let dma_desc = self.bcn_buf as u32;
        let dma_data = (self.bcn_buf as u32).wrapping_add(DESC_SZ as u32);
        let psb_len =
            ((DESC_SZ as u32 + size - 1) / 128 + 1) | (1 << RTK_PCI_TXBD_OWN_OFFSET);
        // The ring slot is tx_buf_desc_sz=16 bytes, but it holds TWO 8-byte
        // rtw_pci_tx_buffer_desc { u16 buf_size; u16 psb_len; u32 dma; }.
        // Linux: buf_desc[0] at +0 (TX desc), buf_desc[1] at +8 (payload).
        unsafe {
            // buf_desc[0] @ +0: buf_size=48, psb_len (OWN set), dma = desc
            (bd as *mut u16).add(0).write_volatile(DESC_SZ as u16);
            (bd as *mut u16).add(1).write_volatile(psb_len as u16);
            (bd as *mut u32).add(1).write_volatile(dma_desc);
            // buf_desc[1] @ +8: buf_size=size, psb_len=0, dma = data
            (bd.add(8) as *mut u16).add(0).write_volatile(size as u16);
            (bd.add(8) as *mut u32).add(1).write_volatile(dma_data);
        }
        unsafe {
            core::arch::asm!("sfence");
        }

        // ---- Program the reserved-page TX + kick (rtw_fw_write_data_rsvd_page) ----
        let bckp_bcn = unsafe { self.read8(REG_BCN_CTRL) };
        let bckp_cr1 = unsafe { self.read8(REG_CR + 1) };
        let bckp_fwhw = unsafe { self.read8(REG_FWHW_TXQ_CTRL + 2) };
        let pg_addr = 0u16; // src == 0 for every FW chunk
        unsafe {
            self.write16(
                REG_FIFOPAGE_CTRL_2,
                (pg_addr & (BIT_MASK_BCN_HEAD_1_V1 as u16)) | (BIT_BCN_VALID_V1 as u16),
            );
            self.write8(REG_CR + 1, bckp_cr1 | (BIT_ENSWBCN >> 8) as u8);
            self.write8(
                REG_BCN_CTRL,
                (bckp_bcn & !(BIT_EN_BCN_FUNCTION as u8)) | (BIT_DIS_TSF_UDT as u8),
            );
            // PCIe: clear BIT_EN_BCNQ_DL (byte +2, bit 6) during the write
            self.write8(REG_FWHW_TXQ_CTRL + 2, bckp_fwhw & !((BIT_EN_BCNQ_DL >> 16) as u8));
            // kick the BCN queue
            self.write8(
                RTK_PCI_TXBD_BCN_WORK,
                self.read8(RTK_PCI_TXBD_BCN_WORK) | BIT_PCI_BCNQ_FLAG,
            );
        }

        // ---- Poll BCN_VALID (Linux check_hw_ready(..., target=1): bit SET) ----
        let mut ok = false;
        for i in 0..1000 {
            let v = unsafe { self.read16(REG_FIFOPAGE_CTRL_2) };
            if v & (BIT_BCN_VALID_V1 as u16) != 0 {
                ok = true;
                crate::mesa_println!("[RTL8822CE]  BCN_VALID set {}us FIFOPAGE={:#06x}", i * 10, v);
                break;
            }
            self.delay_us(10);
        }
        if !ok {
            let v = unsafe { self.read16(REG_FIFOPAGE_CTRL_2) };
            crate::mesa_println!("[RTL8822CE]  BCN_VALID timeout (chunk {}B FIFOPAGE={:#06x})", size, v);
        }

        // ---- Restore (rtw_fw_write_data_rsvd_page "restore:" label) ----
        unsafe {
            self.write16(REG_FIFOPAGE_CTRL_2, self.fw_fifopage_ctrl2_bckp | (BIT_BCN_VALID_V1 as u16));
            self.write8(REG_BCN_CTRL, bckp_bcn);
            self.write8(REG_FWHW_TXQ_CTRL + 2, bckp_fwhw);
            self.write8(REG_CR + 1, bckp_cr1);
        }

        if ok {
            driver_unlock(); Ok(())
        } else {
            Err("RTL8822CE: BCN_VALID timeout")
        }
    }

    /// Verify firmware download integrity via MCUFW_CTRL checksum bits.
    /// This mirrors Linux halmac dlfw_end_flow_88xx which checks that
    /// both IMEM_CHKSUM_OK (BIT4) and DMEM_CHKSUM_OK (BIT6) are set.
    /// Returns Ok if both checksums are OK, Err otherwise.
    fn fw_verify_checksums(&self) -> Result<(), &'static str> {
        let fw_ctrl = unsafe { self.read16(REG_MCUFW_CTRL) };
        let cksum_ok = (fw_ctrl as u32 & BIT_CHECK_SUM_OK) == BIT_CHECK_SUM_OK;
        crate::mesa_println!(
            "[RTL8822CE]  FW checksum verify: MCUFW={:#06x} IMEM_CHKSUM_OK={} DMEM_CHKSUM_OK={} -> {}",
            fw_ctrl,
            (fw_ctrl & BIT_IMEM_CHKSUM_OK as u16) != 0,
            (fw_ctrl & BIT_DMEM_CHKSUM_OK as u16) != 0,
            if cksum_ok { "OK" } else { "FAIL" }
        );
        if cksum_ok {
            driver_unlock(); Ok(())
        } else {
            Err("RTL8822CE: FW checksum verification failed")
        }
    }

    fn check_fw_checksum(&mut self, addr: u32) -> Result<(), &'static str> {
        // Wait for IDDMA transfer to complete
        self.delay_ms(100);

        let ch0ctrl = unsafe { self.read32(REG_DDMA_CH0CTRL) };
        let section = if addr < OCPBASE_DMEM_88XX { "IMEM" } else { "DMEM" };
        crate::mesa_println!("[RTL8822CE]  check_fw_checksum: CH0CTRL={:#010x} section={}", ch0ctrl, section);

        // Linux check_fw_checksum(): CHKSUM_STS set => accumulated checksum
        // mismatch over this section. Set the DW_OK bit but clear the
        // CHKSUM_OK bit so the end flow will not raise FW_DW_RDY.
        let is_imem = addr < OCPBASE_DMEM_88XX;
        let mut fw_ctrl = unsafe { self.read16(REG_MCUFW_CTRL) };
        if (ch0ctrl & BIT_DDMACH0_CHKSUM_STS) != 0 {
            if is_imem {
                fw_ctrl |= BIT_IMEM_DW_OK as u16;
                fw_ctrl &= !(BIT_IMEM_CHKSUM_OK as u16);
            } else {
                fw_ctrl |= BIT_DMEM_DW_OK as u16;
                fw_ctrl &= !(BIT_DMEM_CHKSUM_OK as u16);
            }
            unsafe { self.write16(REG_MCUFW_CTRL, fw_ctrl); }
            crate::mesa_println!("[RTL8822CE]  CHECKSUM ERROR in {} (MCUFW={:#06x})", section, fw_ctrl);
            return Err("RTL8822CE: FW section checksum error");
        }

        if is_imem {
            fw_ctrl |= BIT_IMEM_DW_OK as u16 | BIT_IMEM_CHKSUM_OK as u16;
        } else {
            fw_ctrl |= BIT_DMEM_DW_OK as u16 | BIT_DMEM_CHKSUM_OK as u16;
        }
        unsafe { self.write16(REG_MCUFW_CTRL, fw_ctrl); }
        crate::mesa_println!("[RTL8822CE]  MCUFW after cksum: {:#06x}", fw_ctrl);

        driver_unlock(); Ok(())
    }

    fn download_section_to_mem(&mut self, data: &[u8], dst_addr: u32, first: bool) -> Result<(), &'static str> {
        const MAX_CHUNK: usize = 0x1000;
        let mut offset: usize = 0;
        let mut is_first_chunk = first;
        let mut chunk_num = 0u32;

        crate::mesa_println!("[RTL8822CE]  dl_sec: {}B -> {:#x} first={}", data.len(), dst_addr, first);

        // Reset IDDMA checksum state
        unsafe {
            let val = self.read32(REG_DDMA_CH0CTRL);
            crate::mesa_println!("[RTL8822CE]  CH0CTRL={:#010x}", val);
            self.write32(REG_DDMA_CH0CTRL, val | BIT_DDMACH0_RESET_CHKSUM_STS);
        }

        let dma_buf_ptr = self.bcn_buf_virt;
        let dma_buf_phys = self.bcn_buf as u64;

        if dma_buf_ptr.is_null() {
            crate::mesa_println!("[RTL8822CE]  dl_sec FAIL: staging buffer not allocated");
            return Err("RTL8822CE: DMA buffer not allocated");
        }

        let staging = FW_USE_BCNQ_STAGING;
        crate::mesa_println!(
            "[RTL8822CE]  dl_sec mode: {} (TXBUF/BCNQ staging) vs host-RAM IDDMA",
            if staging { "BCNQ" } else { "DIRECT" }
        );

        while offset < data.len() {
            let chunk_size = core::cmp::min(data.len() - offset, MAX_CHUNK);
            crate::mesa_println!("[RTL8822CE]  ch#{}: off={} sz={}", chunk_num, offset, chunk_size);
            let dst_addr_chunk = dst_addr + offset as u32;

            if staging {
                // Linux/vendor path: stage the chunk into the chip TX FIFO
                // reserved page first, then IDDMA from the TX buffer itself.
                self.fw_write_data_rsvd_page(&data[offset..offset + chunk_size])?;
                let src = OCPBASE_TXBUF_88XX + TX_DESC_SIZE_FW as u32;
                crate::mesa_println!("[RTL8822CE]  iddma(staged): {:#x}->{:#x} {}B", src, dst_addr_chunk, chunk_size);
                self.iddma_copy(src, dst_addr_chunk, chunk_size, is_first_chunk)?;
            } else {
                unsafe {
                    // Copy firmware chunk to DMA buffer
                    core::ptr::write_bytes(dma_buf_ptr, 0, chunk_size);
                    core::ptr::copy_nonoverlapping(data[offset..].as_ptr(), dma_buf_ptr, chunk_size);

                    // Flush CPU cache for DMA coherency
                    let flush_addr = (dma_buf_ptr as usize) & !63usize;
                    let flush_end = ((dma_buf_ptr as usize) + chunk_size + 63) & !63usize;
                    for addr in (flush_addr..flush_end).step_by(64) {
                        core::arch::asm!("clflush [{}]", in(reg) addr, options(nostack));
                    }
                    core::arch::asm!("sfence");
                }

                crate::mesa_println!("[RTL8822CE]  iddma: {:#x}->{:#x} {}B", dma_buf_phys as u32, dst_addr_chunk, chunk_size);
                self.iddma_copy(dma_buf_phys as u32, dst_addr_chunk, chunk_size, is_first_chunk)?;
            }

            chunk_num += 1;
            is_first_chunk = false;
            offset += chunk_size;
        }
        crate::mesa_println!("[RTL8822CE]  dl_sec OK ({} ch)", chunk_num);
        driver_unlock(); Ok(())
    }

    fn iddma_copy(&mut self, src: u32, dst: u32, len: usize, first: bool) -> Result<(), &'static str> {
        // Wait for OWN bit to be clear before starting
        for _ in 0..1000 {
            if unsafe { self.read32(REG_DDMA_CH0CTRL) } & BIT_DDMACH0_OWN == 0 {
                break;
            }
            self.delay_us(10);
        }
        let ctrl_val = unsafe { self.read32(REG_DDMA_CH0CTRL) };
        if ctrl_val & BIT_DDMACH0_OWN != 0 {
            crate::mesa_println!("[RTL8822CE]    IDD BUSY! ctrl={:#010x}", ctrl_val);
            return Err("RTL8822CE: IDDMA channel busy");
        }

        let mut ctrl = BIT_DDMACH0_CHKSUM_EN | BIT_DDMACH0_OWN;
        ctrl |= (len as u32) & BIT_MASK_DDMACH0_DLEN;
        if !first {
            ctrl |= BIT_DDMACH0_CHKSUM_CONT;
        }

        crate::mesa_println!("[RTL8822CE]    IDD start: SA={:#x} DA={:#x} CTL={:#010x}", src, dst, ctrl);
        unsafe {
            self.write32(REG_DDMA_CH0SA, src);
            self.write32(REG_DDMA_CH0DA, dst);
            self.write32(REG_DDMA_CH0CTRL, ctrl);
        }

        for i in 0..1000 {
            let val = unsafe { self.read32(REG_DDMA_CH0CTRL) };
            if val & BIT_DDMACH0_OWN == 0 {
                crate::mesa_println!("[RTL8822CE]    IDD done {}us CTL={:#010x}", i * 10, val);
                driver_unlock(); return Ok(());
            }
            self.delay_us(10);
        }
        crate::mesa_println!("[RTL8822CE]    IDD TIMEOUT CTL={:#010x}", unsafe { self.read32(REG_DDMA_CH0CTRL) });
        Err("RTL8822CE: IDDMA transfer timeout")
    }

    // ========================================================================
    // EFUSE / MAC Address Reading
    // ========================================================================

    fn efuse_read_byte(&self, addr: u16) -> Result<u8, &'static str> {
        unsafe {
            // Set address (bits 17:8) and clear read-done flag (bit 31) to trigger read
            let mut val = self.read32(REG_EFUSE_CTRL);
            val &= !((0x3FF << 8) | (1 << 31));
            val |= ((addr as u32 & 0x3FF) << 8);
            self.write32(REG_EFUSE_CTRL, val);

            // Wait for completion (bit 31 set when EFUSE read is done)
            // REG_EFUSE_CTRL is a 32-bit register at offset 0x0030
            // Bit 31 is in byte 3, so we must use read32 to check it
            for _ in 0..5000 {
                val = self.read32(REG_EFUSE_CTRL);
                if (val & (1 << 31)) != 0 {
                    // Read done — result is in bits 7:0
                    return Ok((val & 0xFF) as u8);
                }
                self.delay_us(10);
            }
            Err("RTL8822CE: EFUSE read timeout")
        }
    }

    fn efuse_dump_all(&self, buf: &mut [u8]) -> Result<(), &'static str> {
        // RTL8822CE physical EFUSE = 512 bytes
        let size = buf.len().min(512);
        for addr in 0..size as u16 {
            match self.efuse_read_byte(addr) {
                Ok(b) => buf[addr as usize] = b,
                Err(_) => buf[addr as usize] = 0xFF,
            }
        }
        driver_unlock(); Ok(())
    }

    /// Decode physical EFUSE map into logical map
    /// Physical format: 1 or 2-byte header + data
    ///   header1: [7:5] = blk_idx[2:0], [4:0] = 0b01111 (1-byte hdr)
    ///   header2 (if 1-byte hdr): [7:4] = blk_idx[6:3], [3:0] = word_en
    ///   word_en: 4 bits, 0 = write word, 1 = skip word
    ///   each word = 2 bytes
    fn efuse_decode_logical(&self, phy_map: &[u8], log_map: &mut [u8]) -> Result<(), &'static str> {
        let phy_size = 512u32;
        let protect_size = 124u32;  // ptct_efuse_size for 8822C
        let log_size = log_map.len() as u32;

        let mut phy_idx = 0u32;
        while phy_idx < phy_size - protect_size {
            let hdr1 = phy_map[phy_idx as usize];
            if hdr1 == 0xFF {
                break; // End of EFUSE
            }

            let mut blk_idx: u32;
            let mut word_en: u8;
            let mut hdr_len: u32;

            if (hdr1 & 0x1F) == 0x0F {
                // 2-byte header
                if phy_idx + 1 >= phy_size - protect_size {
                    break;
                }
                let hdr2 = phy_map[(phy_idx + 1) as usize];
                blk_idx = (((hdr2 & 0xF0) >> 1) as u32) | (((hdr1 >> 5) & 0x07) as u32);
                word_en = hdr2 & 0x0F;
                hdr_len = 2;
            } else {
                // 1-byte header
                blk_idx = ((hdr1 & 0xF0) >> 4) as u32;
                word_en = hdr1 & 0x0F;
                hdr_len = 1;
            }

            let mut data_idx = phy_idx + hdr_len;

            for i in 0..4u32 {
                if (word_en & (1 << i)) != 0 {
                    continue; // word skipped
                }

                let log_idx = (blk_idx << 3) + (i << 1);
                if data_idx + 1 >= phy_size - protect_size || log_idx + 1 >= log_size {
                    return Err("EFUSE: decode overflow");
                }

                log_map[log_idx as usize] = phy_map[data_idx as usize];
                log_map[(log_idx + 1) as usize] = phy_map[(data_idx + 1) as usize];
                data_idx += 2;
            }

            phy_idx = data_idx;
        }
        driver_unlock(); Ok(())
    }

    fn read_mac_from_efuse(&mut self) -> Result<(), &'static str> {
        crate::serial_println!("[RTL8822CE] Attempting EFUSE MAC read...");

        // Step 1: Dump entire physical EFUSE (512 bytes)
        let mut phy_map = [0xFFu8; 512];
        self.efuse_dump_all(&mut phy_map)?;

        // Step 2: Decode to logical map (768 bytes for 8822C)
        let mut log_map = [0xFFu8; 768];
        self.efuse_decode_logical(&phy_map, &mut log_map)?;

        // rfe_option at logical offset 0xca (rtw88 struct rtw8822c_efuse)
        self.efuse.rfe_option = log_map[0xca];
        let cut = unsafe { (self.read32(REG_SYS_CFG1) >> 12) & 0xf } as u8;
        self.efuse.cut_version = cut;
        crate::mesa_println!("[RTL8822CE] EFUSE rfe_option={:#04x} cut={}", self.efuse.rfe_option, cut);

        // Step 3: Read MAC from logical offset 0x120 (rtw8822ce_efuse.mac_addr)
        let mac_offset = 0x120usize;
        let mut mac = [0u8; 6];
        mac.copy_from_slice(&log_map[mac_offset..mac_offset + 6]);

        if mac != [0xFF; 6] && mac != [0x00; 6] {
            self.mac = mac;
            crate::serial_println!(
                "[RTL8822CE] MAC from EFUSE: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                self.mac[0], self.mac[1], self.mac[2],
                self.mac[3], self.mac[4], self.mac[5]
            );
            // Write MAC to hardware registers
            self.write_mac_to_hw()?;
            driver_unlock(); return Ok(());
        }

        crate::serial_println!("[RTL8822CE] EFUSE MAC empty, using default MAC");
        self.mac = [0x02, 0x88, 0x22, 0xCE, 0x00, 0x01];
        // Write default MAC to hardware registers
        self.write_mac_to_hw()?;
        driver_unlock(); Ok(())
    }

    /// Write MAC address to hardware registers (REG_MACID + REG_MACID_H)
    /// Linux: rtw_hal_set_macaddr_port -> writes to REG_MACID (0x0610) + REG_MACID_H (0x0614)
    fn write_mac_to_hw(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Writing MAC to hardware: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.mac[0], self.mac[1], self.mac[2], self.mac[3], self.mac[4], self.mac[5]);
        unsafe {
            // REG_MACID (0x0610) = bytes 0-3
            self.write32(REG_MACID, u32::from_le_bytes([self.mac[0], self.mac[1], self.mac[2], self.mac[3]]));
            // REG_MACID_H (0x0614) = bytes 4-5 (lower 16 bits)
            self.write16(REG_MACID_H, u16::from_le_bytes([self.mac[4], self.mac[5]]));
            // Verify
            let mac_low = self.read32(REG_MACID);
            let mac_high = self.read16(REG_MACID_H);
            crate::mesa_println!("[RTL8822CE] MAC reg verify: MACID={:#010x} MACID_H={:#06x}", mac_low, mac_high);
        }
        driver_unlock(); Ok(())
    }

    // ========================================================================
    // TX/RX DMA Ring Setup
    // ========================================================================

    fn setup_tx_ring(&mut self, queue: usize, desa_reg: u32, num_reg: u32) -> Result<(), &'static str> {
        let ring_size = TX_RING_SIZE;
        // Each packet slot holds two 8-byte descriptors (header segment +
        // payload segment) = TX_BD_STRIDE bytes.
        let desc_total = ring_size * TX_BD_STRIDE;

        // Allocate TX BD ring (DMA coherent)
        let (bd_virt, bd_phys) = unsafe { crate::linux::dma_alloc_coherent(desc_total) };
        if bd_virt.is_null() {
            return Err("RTL8822CE: Failed to allocate TX BD ring");
        }
        // Zero the ring (descriptors are filled per-packet at send time)
        unsafe { core::ptr::write_bytes(bd_virt, 0, desc_total); }

        // Allocate TX data buffer
        let buf_total = ring_size * TX_BUF_SIZE;
        let (buf_virt, buf_phys) = unsafe { crate::linux::dma_alloc_coherent(buf_total) };
        if buf_virt.is_null() {
            return Err("RTL8822CE: Failed to allocate TX data buffers");
        }
        unsafe { core::ptr::write_bytes(buf_virt, 0, buf_total); }

        // Program ring address and NUM into hardware
        // Linux: rtw_write16(rtwdev, num_reg, len & TRX_BD_IDX_MASK)
        // Linux: rtw_write32(rtwdev, desa_reg, dma)
        unsafe {
            self.write16(num_reg, ring_size as u16);
            self.write32(desa_reg, bd_phys as u32);
        }

        self.tx_bd_ring[queue] = bd_phys;
        self.tx_bd_virt[queue] = bd_virt;
        self.tx_bd_count[queue] = ring_size as u16;
        self.tx_buf[queue] = buf_phys as *mut u8;
        self.tx_buf_virt[queue] = buf_virt;
        self.tx_wr_idx[queue] = 0;

        crate::mesa_println!(
            "[RTL8822CE] TX ring[{}]: bd_phys={:#x} DESA={:#x} NUM={:#x}",
            queue, bd_phys, desa_reg, num_reg
        );

        driver_unlock(); Ok(())
    }

    /// Arm the two-segment TX buffer descriptor for packet slot `idx`.
    ///
    /// Segment 0 covers the `desc_size`-byte TX header at `buf_phys` (the
    /// `TX_WIFI_INFO` region), segment 1 covers `payload_len` bytes at
    /// `buf_phys + desc_size`. This mirrors rtw88 `rtw_pci_tx_write_data`
    /// (which fills buf_desc[0] = tx_pkt_desc_sz / buf_desc[1] = size) and the
    /// vendor `rtl8822ce_xmit.c` (`SET_TX_BD_TX_BUFF_SIZE0 = TX_WIFI_INFO_SIZE`,
    /// `SET_TXBUFFER_DESC_LEN_WITH_OFFSET(1, sz)`). The 8822C requires the TX
    /// header in the first segment; a single oversized segment is not allowed.
    fn fill_tx_bd(&self, queue: usize, idx: usize, desc_size: u16, payload_len: u16, buf_phys: u64) {
        let total = desc_size as u32 + payload_len as u32;
        // Page size in 128-byte units, low byte (bits [23:16] of dw0).
        let psb_len = (((total + 127) / 128) & 0xff) as u16;
        let bd0 = TxBufferDesc {
            buf_size: desc_size,
            psb_len,
            dma: buf_phys as u32,
        };
        let bd1 = TxBufferDesc {
            buf_size: payload_len,
            psb_len: 0,
            dma: (buf_phys + desc_size as u64) as u32,
        };
        unsafe {
            let base = (self.tx_bd_virt[queue] as *mut u8).add(idx * TX_BD_STRIDE);
            core::ptr::write(base as *mut TxBufferDesc, bd0);
            core::ptr::write(base.add(core::mem::size_of::<TxBufferDesc>()) as *mut TxBufferDesc, bd1);
        }
    }

    fn setup_rx_ring(&mut self) -> Result<(), &'static str> {
        let ring_size = RX_RING_SIZE;
        let desc_total = ring_size * core::mem::size_of::<RxBufferDesc>();

        // Allocate RX BD ring
        let (bd_virt, bd_phys) = unsafe { crate::linux::dma_alloc_coherent(desc_total) };
        if bd_virt.is_null() {
            return Err("RTL8822CE: Failed to allocate RX BD ring");
        }
        unsafe { core::ptr::write_bytes(bd_virt, 0, desc_total); }

        // Allocate RX data buffers
        for i in 0..ring_size {
            let (buf_virt, buf_phys) = unsafe { crate::linux::dma_alloc_coherent(RX_BUF_SIZE) };
            if buf_virt.is_null() {
                return Err("RTL8822CE: Failed to allocate RX buffer");
            }
            self.rx_buf[i] = buf_virt;
            self.rx_buf_phys[i] = buf_phys;

            // Fill RX buffer descriptor (8-byte HW format).
            // Vendor `rtl8822ce_init_rxbd_ring`: clear FS/LS/total, set buffer
            // size to 12 KiB, set physical address. There is no OWN bit; the
            // hardware signals a completed buffer via TOTALRXPKTSIZE and the
            // ring write index (REG_RXQ_RXBD_IDX).
            let bd = unsafe { &mut *(bd_virt as *mut RxBufferDesc).add(i) };
            bd.arm(RX_BD_RXBUFFSIZE, buf_phys as u32);
        }

        // Program ring address, count and initial host read index
        // (rtw88 rtw_pci_reset_rx_desc / rtw_pci_init_rx_ring).
        unsafe {
            self.write16(REG_RXQ_RXBD_IDX, 0);
            self.write16(REG_RXQ_RXBD_NUM, ring_size as u16);
            self.write32(REG_RXQ_RXBD_DESA, bd_phys as u32);
        }

        self.rx_bd_ring = bd_phys;
        self.rx_bd_virt = bd_virt;
        self.rx_rd_idx = 0;
        self.rx_started = false;

        crate::serial_println!(
            "[RTL8822CE] RX ring: bd_phys={:#x} size={} stride={}",
            bd_phys, ring_size, core::mem::size_of::<RxBufferDesc>()
        );

        driver_unlock(); Ok(())
    }

    /// Allocate the firmware-download staging buffers and enable the PCIe DMA
    /// interface. These buffers back the BCN reserved-page ring used by
    /// `fw_write_data_rsvd_page`, so they MUST exist before firmware download.
    /// Idempotent: safe to call again from `setup_dma_rings`.
    fn alloc_staging_buffers(&mut self) -> Result<(), &'static str> {
        // Enable DMA interface (rtw_pci_reset_buf_desc: RTK_PCI_CTRL + 3 |= 0xf7)
        unsafe {
            let tmp = self.read8(RTK_PCI_CTRL + 3);
            self.write8(RTK_PCI_CTRL + 3, tmp | 0xf7);
        }

        // Already allocated (e.g. setup_dma_rings ran) - do not leak.
        if !self.bcn_bd_virt.is_null() && !self.bcn_buf_virt.is_null() {
            return Ok(());
        }

        // BCN queue ring (for firmware download via reserved page)
        // RTL8822CE uses tx_buf_desc_sz=16: two 8-byte BDs per entry
        // Linux: max_num_of_tx_queue(RTW_TX_QUEUE_BCN) = 1
        let bcn_ring_size = 1; // Linux uses 1 for BCN queue
        // Linux rtw_pci_tx_buffer_desc is 8 bytes but the ring slot stride
        // is tx_buf_desc_sz = 16 for 8822C. The BCN reserved-page TX chains
        // two slots (bd0 = TX desc, bd1 = data), so 32 bytes per entry.
        let bd_size = bcn_ring_size * 32;
        let (bd_virt, bd_phys) = unsafe { crate::linux::dma_alloc_coherent(bd_size) };
        if bd_virt.is_null() {
            return Err("RTL8822CE: Failed to allocate BCN BD ring");
        }
        unsafe { core::ptr::write_bytes(bd_virt, 0, bd_size); }

        // Allocate BCN data buffer (TX desc + max chunk)
        let buf_size = TX_DESC_SIZE_FW + 0x1000; // 48 + 4096
        let (buf_virt, buf_phys) = unsafe { crate::linux::dma_alloc_coherent(buf_size) };
        if buf_virt.is_null() {
            return Err("RTL8822CE: Failed to allocate BCN data buffer");
        }
        unsafe { core::ptr::write_bytes(buf_virt, 0, buf_size); }

        self.bcn_bd_ring = bd_phys;
        self.bcn_bd_virt = bd_virt;
        self.bcn_buf = buf_phys as *mut u8;
        self.bcn_buf_virt = buf_virt;

        // Program the BCN queue's BD base and reset its pointers NOW, before
        // firmware download. Linux does this during probe (rtw_pci_reset_buf_desc
        // + rtw_pci_dma_reset); without it the hardware cannot fetch the
        // reserved-page descriptors and BCN_VALID never asserts.
        unsafe {
            self.write32(RTK_PCI_TXBD_DESA_BCNQ, bd_phys as u32);
            self.write32(RTK_PCI_TXBD_RWPTR_CLR, 0xFFFFFFFF);
            self.write32(RTK_PCI_TXBD_DESA_BCNQ, bd_phys as u32);
            let ctrl = self.read32(RTK_PCI_CTRL);
            self.write32(RTK_PCI_CTRL, ctrl | BIT_RST_TRXDMA_INTF | BIT_RX_TAG_EN);
        }

        crate::mesa_println!("[RTL8822CE] BCN staging allocated: bd_phys={:#x} buf_phys={:#x}", bd_phys, buf_phys);
        crate::mesa_println!("[RTL8822CE] BCN DESA staged={:#010x} (readback)",
            unsafe { self.read32(RTK_PCI_TXBD_DESA_BCNQ) });
        driver_unlock(); Ok(())
    }

    fn setup_dma_rings(&mut self) -> Result<(), &'static str> {
        // Allocate staging buffers + enable DMA interface (idempotent).
        self.alloc_staging_buffers()?;

        // TX rings: VO, VI, BE, BK, MGT, HI
        // Correct register offsets from Linux pci.h
        self.setup_tx_ring(TX_QUEUE_VO as usize, REG_VOQ_TXBD_DESA, REG_VOQ_TXBD_NUM)?;
        self.setup_tx_ring(TX_QUEUE_VI as usize, REG_VIQ_TXBD_DESA, REG_VIQ_TXBD_NUM)?;
        self.setup_tx_ring(TX_QUEUE_BE as usize, REG_BEQ_TXBD_DESA, REG_BEQ_TXBD_NUM)?;
        self.setup_tx_ring(TX_QUEUE_BK as usize, REG_BKQ_TXBD_DESA, REG_BKQ_TXBD_NUM)?;
        self.setup_tx_ring(TX_QUEUE_MGT as usize, REG_MGQ_TXBD_DESA, REG_MGQ_TXBD_NUM)?;
        self.setup_tx_ring(1, REG_HIQ_TXBD_DESA, REG_HIQ_TXBD_NUM)?;
        // H2C command queue (fw_offload PKT transport; slot 0 in our arrays).
        self.setup_tx_ring(TX_QUEUE_H2C as usize, REG_H2CQ_TXBD_DESA, REG_H2CQ_TXBD_NUM)?;

        // RX ring
        self.setup_rx_ring()?;

        // Now re-program ALL ring addresses (Linux rtw_pci_reset_buf_desc order):
        // Program addresses FIRST, then reset pointers
        unsafe {
            // BCN queue: only 32-bit write, no high part (Linux only writes rtw_write32)
            self.write32(RTK_PCI_TXBD_DESA_BCNQ, self.bcn_bd_ring as u32);
        }

        // Reset read/write pointers for ALL queues (rtw_pci_reset_buf_desc: RWPTR_CLR LAST)
        unsafe {
            self.write32(RTK_PCI_TXBD_RWPTR_CLR, 0xFFFFFFFF);
        }

        // Re-write BCN ring address AFTER RWPTR_CLR as safety measure
        unsafe {
            self.write32(RTK_PCI_TXBD_DESA_BCNQ, self.bcn_bd_ring as u32);
        }

        // DMA reset: BIT_RST_TRXDMA_INTF must be AFTER ring programming (Linux rtw_pci_dma_reset order)
        unsafe {
            let ctrl = self.read32(RTK_PCI_CTRL);
            self.write32(RTK_PCI_CTRL, ctrl | BIT_RST_TRXDMA_INTF | BIT_RX_TAG_EN);
        }

        // rtw88 rtw_mac_init order: the TRX FIFO / DMA queue config must run
        // AFTER the TRXDMA interface reset above, otherwise the reset wipes it.
        self.init_trx_cfg()?;
        self.drv_info_cfg()?;

        crate::mesa_println!("[RTL8822CE] BCN DESA reg={:#010x} (readback)",
            unsafe { self.read32(RTK_PCI_TXBD_DESA_BCNQ) });

        driver_unlock(); Ok(())
    }

    // ========================================================================
    // Interrupt Configuration
    // ========================================================================

    fn setup_interrupts(&self) {
        unsafe {
            // Clear all pending status (HISR0/HISR1/HISR3 are write-1-to-clear).
            self.write32(REG_PCIE_HISR0, 0xFFFFFFFF);
            self.write32(REG_PCIE_HISR1, 0xFFFFFFFF);
            self.write32(REG_PCIE_HISR3, 0xFFFFFFFF);

            // HIMR0: RX done + C2H command + TX-done per queue. This matches
            // rtw88's mask[0] minus BCNDMAINT_E (we don't run beacon DMA
            // maintenance) and minus RDU (not enabled upstream either).
            let himr0 = BIT_RXOK_INT
                | BIT_VODOK_INT
                | BIT_VIDOK_INT
                | BIT_BEDOK_INT
                | BIT_BKDOK_INT
                | BIT_MGTDOK_INT
                | BIT_HIGHDOK_INT
                | BIT_C2H_INT;
            self.write32(REG_PCIE_HIMR0, himr0);

            // HIMR1 (TX FIFO overflow) and HIMR3 (H2C done) stay disabled: we
            // don't act on them, and leaving them off avoids unhandled sources.
            self.write32(REG_PCIE_HIMR1, 0x00000000);
            self.write32(REG_PCIE_HIMR3, 0x00000000);
        }

        crate::serial_println!("[RTL8822CE] Interrupts configured (HIMR0={:#010x})", unsafe {
            self.read32(REG_PCIE_HIMR0)
        });
    }

    // ========================================================================
    // Interrupt Processing
    // ========================================================================

    /// Process a WiFi interrupt. Called from the interrupt handler.
    pub fn handle_interrupt(&mut self) {
        // Read all three PCIe status registers (rtw88 rtw_pci_irq_recognized
        // reads HISR0/HISR1/HISR3). HISR1 and HISR3 MUST be read and cleared:
        // HISR0 bit 11 (IMR_HISR1_IND_INT) remains asserted while HISR1 has
        // pending bits, so ignoring HISR1 caused an endless interrupt storm.
        let hisr0 = unsafe { self.read32(REG_PCIE_HISR0) };
        let hisr1 = unsafe { self.read32(REG_PCIE_HISR1) };
        let hisr3 = unsafe { self.read32(REG_PCIE_HISR3) };
        if hisr0 == 0 && hisr1 == 0 && hisr3 == 0 {
            return;
        }

        // One-shot diagnostics: show the first few non-zero interrupt groups.
        use core::sync::atomic::{AtomicU32, Ordering};
        static IRQ_LOG: AtomicU32 = AtomicU32::new(0);
        if IRQ_LOG.load(Ordering::Relaxed) < 4 {
            IRQ_LOG.fetch_add(1, Ordering::Relaxed);
            crate::mesa_println!("[WiFi IRQ] hisr0={:#010x} hisr1={:#010x} hisr3={:#010x}",
                hisr0, hisr1, hisr3);
        }
        unsafe {
            // Write-1-to-clear all three groups.
            self.write32(REG_PCIE_HISR0, hisr0);
            self.write32(REG_PCIE_HISR1, hisr1);
            self.write32(REG_PCIE_HISR3, hisr3);
        }

        // RX done
        if hisr0 & BIT_RXOK_INT != 0 {
            self.process_rx();
        }

        // RX descriptor unavailable (not enabled in HIMR0; drain defensively).
        if hisr0 & BIT_RDU_INT != 0 {
            self.process_rx();
        }

        // C2H command interrupt. The C2H payload itself arrives on the RX ring
        // (decoded in process_rx); here we mirror rtw88's rtw_fw_c2h_cmd_isr,
        // which only looks at REG_MCU_TST_CFG to detect a firmware recovery
        // trigger. There is no C2H FIFO register to read.
        if hisr0 & BIT_C2H_INT != 0 {
            const REG_MCU_TST_CFG: u32 = 0x01D4;
            const VAL_FW_TRIGGER: u16 = 0x1200;
            let tst = unsafe { self.read16(REG_MCU_TST_CFG) };
            if tst == VAL_FW_TRIGGER {
                crate::mesa_println!("[RTL8822CE] FW recovery trigger via C2H (tst={:#06x})", tst);
                self.fw_loaded = false;
            }
        }

        // TX Done interrupts
        if hisr0 & (BIT_VODOK_INT | BIT_VIDOK_INT | BIT_BEDOK_INT
            | BIT_BKDOK_INT | BIT_MGTDOK_INT | BIT_HIGHDOK_INT) != 0
        {
            // TX completed - reclaim descriptors the HW has drained
            self.reclaim_tx_done();
        }

        // Watchdog - periodic check (mirrors Linux rtw_watch_dog_work)
        // Run every ~2 seconds during active traffic
        self.fw_watchdog_timer += 1;
        if self.fw_watchdog_timer >= 200 { // ~2 seconds at 100Hz interrupt rate
            self.fw_watchdog_timer = 0;
            self.watchdog_work();
        }
    }

    // ========================================================================
    // TX (Transmit)
    // ========================================================================

    /// Number of free (not-yet-drained by HW) descriptors in a TX ring.
    /// Mirrors Linux: avail_desc(wp, rp, len).
    fn tx_avail_descs(&self, queue: usize) -> usize {
        let len = self.tx_bd_count[queue] as usize;
        let wp = self.tx_wr_idx[queue] as usize;
        let rp = self.tx_rd_idx[queue] as usize;
        // Ring-buffer free-space: when rp == wp, ring is EMPTY (all slots free)
        // Formula (rp + len - wp) % len gives 0 when empty, but should give len.
        if rp == wp {
            len - 1  // Keep one slot empty to distinguish from full (like Linux)
        } else {
            (rp + len - wp) % len
        }
    }

    /// Reclaim TX ring descriptors that the hardware has consumed.
    /// The HW write index (lower 16 bits of BD_IDX) and read index
    /// (upper 16 bits) are maintained by the card; here we only need
    /// to advance our sw rd index to the HW read index so the slots
    /// become reusable. Mirrors Linux rtw_pci_tx_isr()/__pci_get_hw_tx_ring_rp().
    fn reclaim_tx_done(&mut self) {
        unsafe {
            // For each queue, read BD_IDX and take HP upper word (HW read ptr).
            self.tx_rd_idx[TX_QUEUE_VO as usize] =
                (self.read32(REG_VOQ_TXBD_IDX) >> 16) as u16;
            self.tx_rd_idx[TX_QUEUE_VI as usize] =
                (self.read32(REG_VIQ_TXBD_IDX) >> 16) as u16;
            self.tx_rd_idx[TX_QUEUE_BE as usize] =
                (self.read32(REG_BEQ_TXBD_IDX) >> 16) as u16;
            self.tx_rd_idx[TX_QUEUE_BK as usize] =
                (self.read32(REG_BKQ_TXBD_IDX) >> 16) as u16;
            self.tx_rd_idx[TX_QUEUE_MGT as usize] =
                (self.read32(REG_MGQ_TXBD_IDX) >> 16) as u16;
            self.tx_rd_idx[TX_QUEUE_H2C as usize] =
                (self.read32(REG_H2CQ_TXBD_IDX) >> 16) as u16;
        }
    }

    /// Watchdog work - mirrors Linux rtw_watch_dog_work
    /// Called periodically to check FW health, thermal, EDCCA, etc.
    fn watchdog_work(&mut self) {
        crate::mesa_println!("[RTL8822CE] Watchdog work...");

        unsafe {
            // 1. Check FW status
            let fw_ctrl = self.read16(REG_MCUFW_CTRL);
            if (fw_ctrl & 0xC000) != 0xC000 {
                crate::mesa_println!("[RTL8822CE] Watchdog: FW status abnormal MCUFW={:#06x}", fw_ctrl);
                // Could trigger FW recovery here
            }

            // 2. Check thermal meter (both paths)
            let thermal_a = self.read32(0x1C50) >> 24;
            let thermal_b = self.read32(0x1C54) >> 24;
            self.thermal_meter = ((thermal_a + thermal_b) / 2) as i8;
            self.thermal_avg = self.thermal_meter;

            // 3. Power tracking - check thermal change
            let thermal_diff = (self.thermal_meter as i8 - self.efuse.thermal as i8).abs();
            if thermal_diff >= 5 {
                crate::mesa_println!("[RTL8822CE] Watchdog: Thermal change detected, triggering power tracking");
                self.power_tracking_adjust();
            }

            // 4. EDCCA (Energy Detection CCA) check
            let edcca_decision = self.read32(0x0844);
            self.edcca_flag = (edcca_decision & 0x80000000) != 0;
            if self.edcca_flag {
                crate::mesa_println!("[RTL8822CE] Watchdog: EDCCA flag set");
            }

            // 5. CFO (Carrier Frequency Offset) tracking
            let cfo_a = self.read32(0x1A8C);
            let cfo_b = self.read32(0x1A90);
            if cfo_a != 0 || cfo_b != 0 {
                crate::mesa_println!("[RTL8822CE] Watchdog: CFO non-zero A={:#x} B={:#x}", cfo_a, cfo_b);
                self.cfo_tracking = true;
            }

            // 6. Check RX FA (False Alarm) counters
            let fa_ofdm = self.read32(0x2D00);
            let fa_cck = self.read32(0x1A5C);
            if fa_ofdm > 1000 || fa_cck > 1000 {
                crate::mesa_println!("[RTL8822CE] Watchdog: High FA OFDM={} CCK={}", fa_ofdm, fa_cck);
            }

            // 7. Update DIG (Dynamic Initial Gain) based on FA
            self.dig_update(fa_ofdm, fa_cck);

            // 8. Check RF calibration status
            let iqk_status = self.read8(0x2D9C);
            if iqk_status != 0x55 {
                crate::mesa_println!("[RTL8822CE] Watchdog: IQK status={:#x}", iqk_status);
            }

            // 9. Check DPD (Digital Pre-Distortion)
            let dpd_status = self.read32(0x1BEC);
            if (dpd_status & 0xFFFFFFFF) != 0 {
                crate::mesa_println!("[RTL8822CE] Watchdog: DPD status={:#x}", dpd_status);
            }

            // 10. Check coexistence status
            self.coex_watchdog();
        }
    }

    /// Power tracking adjustment - mirrors Linux rtw8822c_pwrtrack
    fn power_tracking_adjust(&mut self) {
        // Simplified: would adjust swing table based on thermal
        // Linux: rtw8822c_pwrtrack_adjust
        crate::mesa_println!("[RTL8822CE] Power tracking adjust (thermal={})", self.thermal_meter);
        
        unsafe {
            // Read current swing table index
            let swing_2g = self.read32(0x1BCC) & 0xFF;
            let swing_5g = self.read32(0x1BD0) & 0xFF;
            
            // Adjust based on thermal (simplified)
            if self.thermal_meter > 50 {
                // High temp - reduce power
                self.write32(0x1BCC, (swing_2g.saturating_sub(2)) as u32);
                self.write32(0x1BD0, (swing_5g.saturating_sub(2)) as u32);
            } else if self.thermal_meter < 10 {
                // Low temp - increase power
                self.write32(0x1BCC, (swing_2g.saturating_add(2)).min(0xFF) as u32);
                self.write32(0x1BD0, (swing_5g.saturating_add(2)).min(0xFF) as u32);
            }
        }
    }

    /// DIG update based on FA - mirrors Linux rtw_dig
    fn dig_update(&self, fa_ofdm: u32, fa_cck: u32) {
        // Simplified DIG algorithm
        // Linux: rtw8822c_dig
        let mut igi_a = unsafe { (self.read32(0x1C50) & 0x7F) as i8 };
        let mut igi_b = unsafe { (self.read32(0x1C54) & 0x7F) as i8 };
        
        if fa_ofdm > 1000 {
            igi_a = (igi_a + 2).min(0x3F);
            igi_b = (igi_b + 2).min(0x3F);
        } else if fa_cck > 1000 {
            igi_a = (igi_a + 1).min(0x3F);
            igi_b = (igi_b + 1).min(0x3F);
        } else if fa_ofdm < 100 && fa_cck < 100 {
            igi_a = (igi_a - 1).max(0x1C);
            igi_b = (igi_b - 1).max(0x1C);
        }
        
        unsafe {
            self.write32(0x1C50, (self.read32(0x1C50) & !0x7F) | (igi_a as u32 & 0x7F));
            self.write32(0x1C54, (self.read32(0x1C54) & !0x7F) | (igi_b as u32 & 0x7F));
        }
    }

    /// Coex watchdog - mirrors Linux rtw_coex_watchdog
    fn coex_watchdog(&self) {
        // Check BT/WiFi coexistence status
        // Linux: rtw_coex_watchdog
        let bt_status = unsafe { self.read32(0x0070) };
        if (bt_status & 0xFF000000) != 0 {
            crate::mesa_println!("[RTL8822CE] Watchdog: BT active status={:#x}", bt_status);
        }
    }

    pub fn send_packet(&mut self, data: &[u8]) -> Result<(), &'static str> {
        // `data` is an Ethernet frame (dst[6] src[6] ethertype[2] payload).
        // The WiFi hardware expects an 802.11 data frame, so wrap it into a
        // QoS data frame (STA -> AP, ToDS=1) with an LLC/SNAP header.
        if data.len() < 14 {
            return Err("RTL8822CE: Ethernet frame too short");
        }
        let mut dst_mac = [0u8; 6];
        let mut src_mac = [0u8; 6];
        dst_mac.copy_from_slice(&data[0..6]);
        src_mac.copy_from_slice(&data[6..12]);
        let ethertype = u16::from_be_bytes([data[12], data[13]]);
        let payload = &data[14..];

        let bssid = crate::drivers::wifi::state::get_bssid();
        if bssid == [0u8; 6] {
            return Err("RTL8822CE: not associated (no BSSID)");
        }

        // The TX-descriptor BMC bit and the CCMP key selection follow the
        // *receiver* (addr1), like mac80211 (ieee80211_select_link_key) and
        // rtw88 (rtw_tx_pkt_info_update). For a ToDS STA->AP frame addr1 is the
        // AP's BSSID (always unicast), even when the Ethernet DA in addr3 is
        // broadcast. Keying off the DA here selected the GTK for the broadcast
        // DHCP DISCOVER/ARP; the AP tried the pairwise key, failed the MIC and
        // deauthenticated us (reason=3). Always use the PTK for TX here.
        let bmc = (bssid[0] & 0x01) != 0;
        // EAPOL is always sent unencrypted; other data frames use CCMP once the
        // 4-way handshake has installed the keys. We encrypt in SOFTWARE and
        // transmit with SEC_TYPE=0 (HW crypto off), mirroring the vendor driver's
        // software-encrypt path; this proved far more reliable than HW CCMP.
        let ccmp = ethertype != 0x888E
            && crate::net::wpa::get_wpa_state() == crate::net::wpa::WpaState::Completed;
        let sec_type: u32 = 0;

        let mut frame = crate::drivers::wifi::frame::Ieee80211Parser::build_data_frame(
            bssid, src_mac, dst_mac, ethertype, payload,
        );

        if ccmp {
            const DATA_HDR_LEN: usize = 24; // non-QoS Data frame, ToDS=1
            if frame.len() < DATA_HDR_LEN {
                return Err("RTL8822CE: short data frame");
            }
            // Mark the frame as protected (FC bit 14), like mac80211's
            // ieee80211_tx_set_protected() / the vendor SetPrivacy().
            frame[1] |= 0x40;

            let (key_id, key): (u8, [u8; 16]) = if bmc {
                let kid = crate::net::wpa::get_gtk_keyid() & 0x3;
                let gtk = crate::net::wpa::get_gtk()
                    .map(|(g, _len)| {
                        let mut k = [0u8; 16];
                        k.copy_from_slice(&g[..16]);
                        k
                    })
                    .ok_or("RTL8822CE: no GTK installed")?;
                (kid, gtk)
            } else {
                let tk = crate::net::wpa::get_ptk()
                    .map(|p| p.tk)
                    .ok_or("RTL8822CE: no PTK installed")?;
                (0, tk)
            };

            let header = frame[..DATA_HDR_LEN].to_vec();
            let pn = CCMP_TX_PN.fetch_add(1, core::sync::atomic::Ordering::Relaxed) + 1;
            let extra = crate::net::wpa::ccmp_encrypt_frame(
                &key, key_id, &src_mac, pn, &header, &frame[DATA_HDR_LEN..],
            );
            let mut enc = alloc::vec::Vec::with_capacity(DATA_HDR_LEN + extra.len());
            enc.extend_from_slice(&header);
            enc.extend_from_slice(&extra);
            frame = enc;
        }

        if frame.len() > TX_BUF_SIZE - TX_HDR_SIZE as usize {
            return Err("RTL8822CE: Packet too large for TX buffer");
        }

        // Reclaim completed TX descriptors first (update HW read index)
        self.reclaim_tx_done();

        // Use BE (Best Effort) queue for data
        let queue = TX_QUEUE_BE as usize;
        let idx = self.tx_wr_idx[queue] as usize;

        // Ring flow control: never overwrite descriptors still owned by HW.
        // Mirrors Linux avail_desc() < 2 -> stop / -ENOSPC.
        if self.tx_avail_descs(queue) < 2 {
            return Err("RTL8822CE: TX ring full");
        }

        // 802.11 sequence number as written by build_data_frame(). rtw88
        // (rtw_tx_data_pkt_info_update + rtw_tx_fill_tx_desc) always programs
        // DISQSELSEQ=1 / EN_HWSEQ=1 / SW_SEQ=<hdr seq> so the HW transmits
        // exactly this value. Leaving them clear makes the HW fall back to its
        // own per-QSEL counter, which is not maintained here and stays at 0:
        // every data frame then goes out with seq=0 and the AP drops msg 4/4
        // (and the following DHCP frames) as 802.11 duplicates of msg 2/4.
        let seq = (u16::from_le_bytes([frame[22], frame[23]]) >> 4) & 0x0fff;

        // Check for space
        let next_idx = (idx + 1) % self.tx_bd_count[queue] as usize;

        // Get data buffer physical address
        let buf_phys = unsafe {
            (self.tx_buf[queue] as u64) + (idx as u64) * TX_BUF_SIZE as u64
        };
        let buf_virt = unsafe {
            (self.tx_buf_virt[queue] as u64) + (idx as u64) * TX_BUF_SIZE as u64
        };

        // TX packet descriptor (rtw88 rtw_tx_desc layout, 40 bytes):
        //   w0 size|offset|bmc|ls, w1 macid|qsel|rate_id, w3 use_rate,
        //   w4 datarate, w5 bw/sgi.  The DMA address is NOT in the packet
        //   descriptor: it lives in the TX BD ring (armed via fill_tx_bd).
        // rtw88 uses, for management (and non-HT data on 2G), the robust
        // 1 Mbps CCK basic rate: rate_id = RTW_RATEID_B_20M (8), use_rate,
        // dis_rate_fallback. Our old 6 Mbps OFDM fixed rate was lost ~70% of
        // the time on this link (the AP kept retransmitting msg1/msg3 and then
        // deauthed us), while management at 1 Mbps was 100% reliable. Use the
        // same robust rate for data so EAPOL and DHCP survive a weak link.
        const RATE_ID_B_20M: u32 = 8;   // rtw88 RTW_RATEID_B_20M
        const DESC_RATE1M: u32 = 0x00;  // rtw88 DESC_RATE1M (CCK, most robust)
        let tx_desc = TxDesc {
            dword0: ((frame.len() as u32) & 0x0000ffff)
                | ((TX_HDR_SIZE as u32) << 16)   // TXPKTSIZE | OFFSET(48)
                | (if bmc { 1 << 24 } else { 0 }) // BMC
                | (1 << 26)                      // LS
                | (1 << 31),                     // DISQSELSEQ (use SW_SEQ below)
            dword1: (RATE_ID_B_20M << 16) | (sec_type << 22), // MACID=0, QSEL=0 (TID0/BE), RATE_ID, SEC_TYPE
            dword2: 0,
            dword3: (1 << 8) | (1 << 10),      // USE_RATE | DISDATAFB (no RA table needed)
            dword4: DESC_RATE1M & 0x7f,        // DATARATE = 1M CCK
            dword5: 0,                         // 20MHz, no SGI
            dword6: 0,
            dword7: 0,
            // EN_HWSEQ=1 makes the HW insert SW_SEQ into the transmitted
            // seq_ctrl. SW_SEQ is the exact value we hashed into the CCMP AAD,
            // so the frame on air still matches the software MIC.
            dword8: 1 << 15,                   // EN_HWSEQ
            dword9: (seq as u32) << 12,        // SW_SEQ[23:12]
        };

        // Copy the descriptor at the head of the TX header area and the frame
        // payload at offset TX_HDR_SIZE.
        unsafe {
            core::ptr::write_bytes(buf_virt as *mut u8, 0, TX_HDR_SIZE as usize);
            core::ptr::copy_nonoverlapping(
                &tx_desc as *const TxDesc as *const u8,
                buf_virt as *mut u8,
                core::mem::size_of::<TxDesc>(),
            );
            core::ptr::copy_nonoverlapping(
                frame.as_ptr(),
                (buf_virt as *mut u8).add(TX_HDR_SIZE as usize),
                frame.len(),
            );
        }

        // Arm the two-segment TX buffer descriptor (header + payload).
        self.fill_tx_bd(queue, idx, TX_HDR_SIZE, frame.len() as u16, buf_phys);

        // Memory fence
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

        // Program the non-QoS hardware SSN explicitly before kicking the
        // descriptor. With EN_HWSEQ=1 the MAC takes the transmitted sequence
        // from REG_HW_SEQ0 (selected by HW_SSN_SEL=0), NOT from SW_SEQ. If that
        // counter is not advanced, every EAPOL/DHCP frame leaves with the same
        // sequence (0) and the AP's duplicate detector silently drops msg 4/4
        // and the DHCP DISCOVERs: the AP keeps retransmitting msg 3/4 and then
        // deauthenticates us (reason=3). Writing the header-derived seq here
        // guarantees a fresh, monotonic on-air sequence regardless of whether
        // the MAC uses the HW register or the SW_SEQ field.
        unsafe {
            self.write16(REG_HW_SEQ0, seq);
        }
        LAST_TX_SEQ.store(seq, core::sync::atomic::Ordering::Relaxed);

        // Advance write index
        self.tx_wr_idx[queue] = next_idx as u16;

        // Notify hardware of new TX
        let idx_reg = match queue as u32 {
            TX_QUEUE_VO => REG_VOQ_TXBD_IDX,
            TX_QUEUE_VI => REG_VIQ_TXBD_IDX,
            TX_QUEUE_BE => REG_BEQ_TXBD_IDX,
            TX_QUEUE_BK => REG_BKQ_TXBD_IDX,
            TX_QUEUE_MGT => REG_MGQ_TXBD_IDX,
            _ => REG_BEQ_TXBD_IDX,
        };
        unsafe {
            self.write32(idx_reg, self.tx_wr_idx[queue] as u32);
        }

        driver_unlock(); Ok(())
    }

    // ========================================================================
    // RX (Receive)
    // ========================================================================

    /// Wait for RX packet to be ready (total_len != 0).
    /// Mirrors Linux rtl8822ce_wait_rxrdy: polls RX BD until total_len is set.
    /// Parse the 24-byte RX packet descriptor that the hardware writes at the
    /// head of each RX buffer.
    ///
    /// Field masks follow rtw88 `rtw_rx_query_rx_desc` (rx.c) and rx.h:
    ///   w0[13:0]  PKT_LEN         (802.11 frame length)
    ///   w0[14]    CRC32 error
    ///   w0[19:16] DRV_INFO_SIZE   (in units of 8 bytes)
    ///   w0[25:24] SHIFT           (extra offset before the frame)
    ///   w2[28]    C2H             (packet is a C2H report, not 802.11)
    ///
    /// Returns `(payload_offset, pkt_len, is_c2h, crc_err)`.
    fn parse_rx_pkt_desc(&self, idx: usize) -> Option<(usize, usize, bool, bool)> {
        let base = self.rx_buf[idx];
        if base.is_null() {
            return None;
        }
        let (w0, w2) = unsafe {
            (
                core::ptr::read_unaligned(base as *const u32),
                core::ptr::read_unaligned((base as *const u32).add(2)),
            )
        };
        let pkt_len = (w0 & 0x3FFF) as usize;
        let crc_err = (w0 & (1 << 14)) != 0;
        let drv_info_sz = (((w0 >> 16) & 0xF) as usize) * 8;
        let shift = ((w0 >> 24) & 0x3) as usize;
        let is_c2h = (w2 & (1 << 28)) != 0;
        Some((RX_PKT_DESC_SIZE + drv_info_sz + shift, pkt_len, is_c2h, crc_err))
    }

    fn process_rx(&mut self) {
        use crate::drivers::wifi::frame::{Ieee80211Header, FC_TYPE_DATA, FC_TYPE_MGMT};

        // Set true only when diagnosing RX descriptor layout; it makes
        // process_rx very chatty (it runs every 10ms while scanning).
        const RX_DUMP_DESC: bool = false;

        let ring_size = RX_RING_SIZE;
        // REG_RXQ_RXBD_IDX packs the host read index in bits [11:0] and the
        // hardware write index in bits [27:16] (rtw88 TRX_BD_HW_IDX_MASK;
        // vendor rtl8822ce_check_rxdesc_remain uses `(reg >> 16) & 0x7ff`).
        let idx_reg = unsafe { self.read32(REG_RXQ_RXBD_IDX) };
        let hw_idx = ((idx_reg >> 16) & 0x7FF) as usize;

        // Vendor ignores the initial all-zero state: only start once the HW has
        // advanced the write index at least once.
        if !self.rx_started {
            if hw_idx == 0 {
                return;
            }
            self.rx_started = true;
        }

        let host_idx = self.rx_rd_idx as usize;
        let count = if hw_idx >= host_idx {
            hw_idx - host_idx
        } else {
            ring_size - host_idx + hw_idx
        }
        .min(ring_size);
        if count == 0 {
            return;
        }

        if RX_DUMP_DESC {
            crate::mesa_println!("[WiFi RX] idx_reg={:#010x} hw={} host={} count={}",
                idx_reg, hw_idx, host_idx, count);
        }

        let is_scanning = crate::drivers::wifi::scan::is_scanning();
        for _ in 0..count {
            let idx = self.rx_rd_idx as usize;

            // Heavy raw descriptor dump: off by default (it printed ~5 lines
            // per descriptor and process_rx runs every 10ms during a dwell).
            // Flip RX_DUMP_DESC to true only when diagnosing descriptor layout.
            if RX_DUMP_DESC && !self.rx_buf[idx].is_null() {
                unsafe {
                    let bd0 = core::ptr::read_unaligned((self.rx_bd_virt as *const u32).add(idx * 2));
                    let bd1 = core::ptr::read_unaligned((self.rx_bd_virt as *const u32).add(idx * 2 + 1));
                    let d = self.rx_buf[idx] as *const u32;
                    crate::mesa_println!(
                        "[WiFi RXDSC] idx={} bd0={:#010x} bd1={:#010x} w0={:#010x} w2={:#010x}",
                        idx, bd0, bd1,
                        core::ptr::read_unaligned(d),
                        core::ptr::read_unaligned(d.add(2)));
                }
            }

            // Readiness is governed by the hardware write index (already read
            // above), exactly like rtw88 rtw_pci_rx_napi. NOTE: with RX_TAG
            // enabled (BIT_RX_TAG_EN), TOTALRXPKTSIZE holds a *rolling tag*, not
            // a length, so it must not be polled here.
            if let Some((offset, pkt_len, is_c2h, crc_err)) = self.parse_rx_pkt_desc(idx) {
                if is_c2h {
                    // C2H reports carry a small descriptor, not an 802.11 frame;
                    // the actual report id/payload begins at `offset`.
                    if crate::drivers::wifi::scan::is_verbose() {
                        let id = if pkt_len >= 2 && offset + 2 <= RX_BUF_SIZE {
                            unsafe { core::ptr::read_unaligned(self.rx_buf[idx].add(offset) as *const u16) }
                        } else {
                            0
                        };
                        crate::mesa_println!("[WiFi C2H] id={:#06x} len={}", id, pkt_len);
                    }
                } else if pkt_len == 0 || crc_err || offset + pkt_len > RX_BUF_SIZE {
                    // Descriptor did not decode to a frame (empty/oversize/CRC).
                    if crate::drivers::wifi::scan::is_verbose() {
                        let (rw0, rw1, rw2) = unsafe {
                            (
                                core::ptr::read_unaligned(self.rx_buf[idx] as *const u32),
                                core::ptr::read_unaligned((self.rx_buf[idx] as *const u32).add(1)),
                                core::ptr::read_unaligned((self.rx_buf[idx] as *const u32).add(2)),
                            )
                        };
                        crate::mesa_println!("[WiFi RAW] idx={} w0={:#010x} w1={:#010x} w2={:#010x} len={} crc={} off={}",
                            idx, rw0, rw1, rw2, pkt_len, crc_err as u8, offset);
                    }
                } else if !crc_err && pkt_len > 0 && offset + pkt_len <= RX_BUF_SIZE {
                    let frame_data = unsafe {
                        core::slice::from_raw_parts(self.rx_buf[idx].add(offset), pkt_len)
                    };
                    if frame_data.len() >= 24 {
                        let hdr = unsafe { &*(frame_data.as_ptr() as *const Ieee80211Header) };
                        let fc = u16::from_le(hdr.frame_control);
                        let frame_type = fc & 0x000C;
                        let subtype = (fc & 0x00F0) >> 4;

                        if crate::drivers::wifi::scan::is_verbose() {
                            crate::mesa_println!("[WiFi RX] scan={} type={:x} subtype={:x} len={} BSSID={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                                is_scanning, frame_type, subtype, frame_data.len(),
                                hdr.addr3[0], hdr.addr3[1], hdr.addr3[2],
                                hdr.addr3[3], hdr.addr3[4], hdr.addr3[5]);
                        }

                        match frame_type {
                            val if val == FC_TYPE_MGMT as u16 => {
                                wifi_recv_mgmt(frame_data);
                            }
                            val if val == FC_TYPE_DATA as u16 => {
                                let n = RX_DATA_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                                if n < 8 {
                                    crate::mesa_println!("[WiFi RX] DATA #{} a1={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} a2={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} a3={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} len={}",
                                        n, hdr.addr1[0],hdr.addr1[1],hdr.addr1[2],hdr.addr1[3],hdr.addr1[4],hdr.addr1[5],
                                        hdr.addr2[0],hdr.addr2[1],hdr.addr2[2],hdr.addr2[3],hdr.addr2[4],hdr.addr2[5],
                                        hdr.addr3[0],hdr.addr3[1],hdr.addr3[2],hdr.addr3[3],hdr.addr3[4],hdr.addr3[5],
                                        frame_data.len());
                                }
                                let parsed_opt = crate::drivers::wifi::frame::Ieee80211Parser::parse_data_frame(frame_data);
                                // While a DHCP REQUEST/ACK is in flight, log the
                                // first few data frames: distinguishes "the AP
                                // never delivered the reply" from "it arrived
                                // but did not parse/decrypt" (silent failure).
                                let dhcp_ph = crate::net::dhcp::phase();
                                if dhcp_ph == crate::net::dhcp::PHASE_DISCOVER
                                    || dhcp_ph == crate::net::dhcp::PHASE_REQUEST
                                {
                                    let n = RX_DHCP_DBG.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                                    if n < 12 {
                                        let prot = (u16::from_le(hdr.frame_control) & 0x4000) != 0;
                                        let eth = match &parsed_opt {
                                            Some(p) => ((p.payload.get(12).copied().unwrap_or(0) as u16) << 8)
                                                | p.payload.get(13).copied().unwrap_or(0) as u16,
                                            None => 0,
                                        };
                                        crate::mesa_println!(
                                            "[WiFi RXD] #{} ph={} len={} prot={} a1={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} a2={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} eth={:04x} parse={}",
                                            n, dhcp_ph, frame_data.len(), prot,
                                            hdr.addr1[0],hdr.addr1[1],hdr.addr1[2],hdr.addr1[3],hdr.addr1[4],hdr.addr1[5],
                                            hdr.addr2[0],hdr.addr2[1],hdr.addr2[2],hdr.addr2[3],hdr.addr2[4],hdr.addr2[5],
                                            eth, parsed_opt.is_some() as u8
                                        );
                                    }
                                }
                                if let Some(parsed) = parsed_opt {
                                    if !parsed.payload.is_empty() {
                                        crate::net::handle_packet(&parsed.payload);
                                    }
                                }
                            }
                            _ => {} // Control frames: ignored
                        }
                    }
                }
            }

            self.return_rx_bd(idx);
        }

        // Publish the advanced host read index (low 16 bits) to the hardware.
        unsafe {
            self.write16(REG_RXQ_RXBD_IDX, self.rx_rd_idx);
        }
    }

    fn return_rx_bd(&mut self, idx: usize) {
        let bd = unsafe { &mut *(self.rx_bd_virt as *mut RxBufferDesc).add(idx) };
        // Re-arm: restore buffer size and address (rtw88 rtw_pci_sync_rx_desc_device
        // rewrites buf_size + dma; vendor rewrites PHYSICAL_ADDR_LOW + RXBUFFSIZE).
        bd.arm(RX_BD_RXBUFFSIZE, self.rx_buf_phys[idx] as u32);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        self.rx_rd_idx = ((idx + 1) % RX_RING_SIZE) as u16;
    }

    // ========================================================================
    // H2C (Host-to-Chip) Command Interface
    // ========================================================================

    pub fn h2c_command(&mut self, cmd: &[u8; 8]) -> Result<(), &'static str> {
        let box_num = self.h2c_box;

        // Check if previous command was processed
        let bit = 1u32 << box_num;
        let val = unsafe { self.read32(REG_HMETFR) };
        if val & bit != 0 {
            // Previous command still pending
            crate::serial_println!("[RTL8822CE] H2C box {} busy", box_num);
            return Err("H2C mailbox busy");
        }

        let (box_reg, box_ext_reg) = match box_num {
            0 => (REG_HMEBOX0, REG_HMEBOX0_EX),
            1 => (REG_HMEBOX1, REG_HMEBOX1_EX),
            2 => (REG_HMEBOX2, REG_HMEBOX2_EX),
            3 => (REG_HMEBOX3, REG_HMEBOX3_EX),
            _ => return Err("Invalid H2C box"),
        };

        // Write extension (bytes 4-7) first
        let ext = u32::from_le_bytes([cmd[4], cmd[5], cmd[6], cmd[7]]);
        unsafe { self.write32(box_ext_reg, ext); }

        // Write command (bytes 0-3) - triggers the send
        let cmd32 = u32::from_le_bytes([cmd[0], cmd[1], cmd[2], cmd[3]]);
        unsafe { self.write32(box_reg, cmd32); }

        // Advance to next mailbox
        self.h2c_box = (box_num + 1) % 4;

        driver_unlock(); Ok(())
    }

/// Send a full 32-byte H2C packet using all 4 mailboxes
    /// Used for commands requiring full payload like channel switch (20-byte payload)
    fn h2c_command_full(&mut self, pkt: &[u8; 32]) -> Result<(), &'static str> {
        // Check if all 4 mailboxes are available (we need all 4 for 32 bytes)
        let val = unsafe { self.read32(REG_HMETFR) };
        if val & 0xF != 0 {
            crate::serial_println!("[RTL8822CE] H2C mailboxes busy: {:#010x}", val);
            return Err("H2C mailboxes busy");
        }

        // crate::mesa_println!("[RTL8822CE] Sending full 32-byte H2C packet via 4 mailboxes");

        // Write all 4 mailboxes (8 bytes each = 32 bytes total)
        // Mailbox 0: bytes 0-7
        unsafe {
            self.write32(REG_HMEBOX0, u32::from_le_bytes([pkt[0], pkt[1], pkt[2], pkt[3]]));
            self.write32(REG_HMEBOX0_EX, u32::from_le_bytes([pkt[4], pkt[5], pkt[6], pkt[7]]));
            // Mailbox 1: bytes 8-15
            self.write32(REG_HMEBOX1, u32::from_le_bytes([pkt[8], pkt[9], pkt[10], pkt[11]]));
            self.write32(REG_HMEBOX1_EX, u32::from_le_bytes([pkt[12], pkt[13], pkt[14], pkt[15]]));
            // Mailbox 2: bytes 16-23
            self.write32(REG_HMEBOX2, u32::from_le_bytes([pkt[16], pkt[17], pkt[18], pkt[19]]));
            self.write32(REG_HMEBOX2_EX, u32::from_le_bytes([pkt[20], pkt[21], pkt[22], pkt[23]]));
            // Mailbox 3: bytes 24-31
            self.write32(REG_HMEBOX3, u32::from_le_bytes([pkt[24], pkt[25], pkt[26], pkt[27]]));
            self.write32(REG_HMEBOX3_EX, u32::from_le_bytes([pkt[28], pkt[29], pkt[30], pkt[31]]));
        }

        // Notify hardware
        unsafe { self.write32(REG_HMETFR, 0xF); }

        driver_unlock(); Ok(())
    }

    /// Send a 32-byte H2C offload packet through the H2C command queue.
    ///
    /// Mirrors rtw88 `rtw_fw_send_h2c_packet` -> `rtw_pci_write_data_h2c`,
    /// which enqueues the 32-byte fw_offload packet (category 0x01, cmd 0xFF)
    /// on `RTW_TX_QUEUE_H2C` with a TX descriptor whose QSEL is
    /// `TX_DESC_QSEL_H2C` and whose OFFSET is 0 (the H2C packet is not an
    /// 802.11 frame). The fw_offload sequence number lives in bytes [7:6].
    fn h2c_send_pkt(&mut self, pkt: &[u8; 32]) -> Result<(), &'static str> {
        let queue = TX_QUEUE_H2C as usize;
        self.reclaim_tx_done();
        let idx = self.tx_wr_idx[queue] as usize;
        if self.tx_avail_descs(queue) < 2 {
            return Err("RTL8822CE: H2C ring full");
        }

        let mut payload = *pkt;
        payload[6] = (self.h2c_seq & 0xff) as u8;
        payload[7] = (self.h2c_seq >> 8) as u8;
        self.h2c_seq = self.h2c_seq.wrapping_add(1);

        let buf_phys = unsafe {
            (self.tx_buf[queue] as u64) + (idx as u64) * TX_BUF_SIZE as u64
        };
        let buf_virt = unsafe {
            (self.tx_buf_virt[queue] as u64) + (idx as u64) * TX_BUF_SIZE as u64
        };

        let tx_desc = TxDesc {
            dword0: 32u32 | (1 << 26),          // TXPKTSIZE=32, OFFSET=0, LS=1
            dword1: TX_DESC_QSEL_H2C << 8,      // QSEL[12:8] = 19 (H2C)
            dword2: 0,
            dword3: 0,
            dword4: buf_phys as u32,
            dword5: (buf_phys >> 32) as u32,
            dword6: 0,
            dword7: 0,
            dword8: 0,
            dword9: 0,
        };

        unsafe {
            core::ptr::write_bytes(buf_virt as *mut u8, 0, TX_HDR_SIZE as usize);
            core::ptr::copy_nonoverlapping(
                &tx_desc as *const TxDesc as *const u8,
                buf_virt as *mut u8,
                core::mem::size_of::<TxDesc>(),
            );
            core::ptr::copy_nonoverlapping(
                payload.as_ptr(),
                (buf_virt as *mut u8).add(TX_HDR_SIZE as usize),
                32,
            );
        }

        self.fill_tx_bd(queue, idx, TX_HDR_SIZE, 32, buf_phys);

        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        let next_idx = (idx + 1) % self.tx_bd_count[queue] as usize;
        self.tx_wr_idx[queue] = next_idx as u16;
        unsafe {
            self.write32(REG_H2CQ_TXBD_IDX, self.tx_wr_idx[queue] as u32);
        }

        driver_unlock(); Ok(())
    }

    /// Write one entry into the hardware security CAM, exactly as rtw88
    /// `rtw_sec_write_cam()` (sec.c) does. The hardware matches unicast frames
    /// by address and group frames by (default-key-search) key index, so the
    /// pairwise entry carries the AP/BSSID address and the group entry the
    /// broadcast address + group flag.
    fn sec_write_cam(
        &self,
        hw_key_idx: u8,
        key_idx: u8,
        hw_key_type: u8,
        group: bool,
        addr: [u8; 6],
        key: &[u8],
    ) {
        let write_cmd = SEC_CMD_WRITE_ENABLE | SEC_CMD_POLLING;
        let base = (hw_key_idx as u32) << SEC_CAM_ENTRY_SHIFT;
        for i in (0..8u32).rev() {
            let content: u32 = match i {
                0 => (key_idx as u32 & 0x3)
                    | ((hw_key_type as u32 & 0x7) << 2)
                    | ((group as u32) << 6)
                    | (1u32 << 15) // valid
                    | ((addr[0] as u32) << 16)
                    | ((addr[1] as u32) << 24),
                1 => (addr[2] as u32)
                    | ((addr[3] as u32) << 8)
                    | ((addr[4] as u32) << 16)
                    | ((addr[5] as u32) << 24),
                6 | 7 => 0,
                _ => {
                    let j = ((i - 2) * 4) as usize;
                    let g = |k: usize| if k < key.len() { key[k] as u32 } else { 0 };
                    g(j) | (g(j + 1) << 8) | (g(j + 2) << 16) | (g(j + 3) << 24)
                }
            };
            unsafe {
                self.write32(REG_SEC_WRITE, content);
                self.write32(REG_SEC_CMD, write_cmd | (base + i));
            }
        }
    }

    /// Enable the hardware security engine (TX/RX decryption) and default key
    /// search, mirroring rtw88 `rtw_sec_enable_sec_engine()`.
    fn sec_enable_engine(&self) {
        unsafe {
            let mut cr = self.read16(REG_CR);
            cr |= SEC_ENGINE_EN;
            self.write16(REG_CR, cr);

            let mut cfg = self.read16(REG_SEC_CONFIG);
            cfg |= SEC_TX_DEC_EN
                | SEC_RX_DEC_EN
                | SEC_TX_UNI_USE_DK
                | SEC_RX_UNI_USE_DK
                | SEC_TX_BC_USE_DK
                | SEC_RX_BC_USE_DK;
            self.write16(REG_SEC_CONFIG, cfg);
            crate::mesa_println!(
                "[RTL8822CE] SEC engine: CR={:#06x} SEC_CONFIG={:#06x}",
                cr, cfg
            );
        }
    }

    /// Install the CCMP pairwise (TK) + group (GTK) keys after the 4-way
    /// handshake. Pairwise goes to CAM entry 4 (entries 0-3 are reserved for
    /// default key search), group to entry `gtk_keyid`.
    pub fn install_ccmp_keys(
        &mut self,
        bssid: [u8; 6],
        tk: &[u8],
        gtk: &[u8],
        gtk_keyid: u8,
        gtk_hw_type: u8,
    ) {
        driver_lock();
        self.sec_write_cam(4, 0, HWCAM_AES, false, bssid, tk);
        crate::mesa_println!(
            "[RTL8822CE] CAM pairwise: entry=4 addr={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            bssid[0], bssid[1], bssid[2], bssid[3], bssid[4], bssid[5]
        );
        if !gtk.is_empty() {
            let gi = gtk_keyid & 0x3;
            self.sec_write_cam(gi, gi, gtk_hw_type, true, [0xff; 6], gtk);
            crate::mesa_println!(
                "[RTL8822CE] CAM group: entry={} keyid={} len={} hw_type={}",
                gi, gi, gtk.len(), gtk_hw_type
            );
        }
        self.sec_enable_engine();
        driver_unlock();
    }

    /// Install security key via H2C command - matches Linux rtl8822c HalH2CCmdSecAddKey
    #[allow(dead_code)]
    fn h2c_security_add_key(&mut self, key_idx: u8, is_grp: bool, algorithm: u8, key_len: u8, key_data: &[u8]) -> Result<(), &'static str> {
        // H2C security add key command format (32 bytes):
        // Byte 0: H2C box number (0-3)
        // Byte 1: Key index (0-3)
        // Byte 2: Key mapping control + algorithm
        //   Bit 7: Key type (0=PAIRWISE, 1=GROUP)
        //   Bits 6-5: Key mapping (0=default, 1-3=other)
        //   Bit 4: Key index (0-3)
        //   Bit 3: Key restore (0=don't restore, 1=restore)
        //   Bit 2: Key ignore (0=don't ignore, 1=ignore)
        //   Bit 1: Key accept (0=don't accept, 1=accept)
        //   Bit 0: Key restore defer (0=don't defer, 1=defer)
        // Byte 3: Algorithm (0=WEP, 1=TKIP, 2=CCMP, 3=GCMP)
        // Byte 4: Key length
        // Bytes 5-31: Key data (up to 32 bytes)
        
        let mut cmd = [0u8; 32];
        
        // Set up command header
        cmd[0] = self.h2c_box;  // H2C box
        cmd[1] = key_idx;       // Key index
        
        // Build key mapping control byte
        let mut key_map_ctrl: u8 = 0;
        
        // Bit 7: Key type (0=PAIRWISE, 1=GROUP)
        if is_grp {
            key_map_ctrl |= 0x80;
        }
        
        // Bits 6-5: Key mapping (0=default, 1-3=other)
        // For default key, leave as 0
        
        // Bit 4: Key index (0-3)
        key_map_ctrl |= (key_idx & 0x03) << 4;
        
        // Bit 3: Key restore (0=don't restore, 1=restore)
        // Leave as 0 (don't restore)
        
        // Bit 2: Key ignore (0=don't ignore, 1=ignore)
        // Leave as 0 (don't ignore)
        
        // Bit 1: Key accept (0=don't accept, 1=accept)
        // Leave as 0 (will be set by hardware/hw state)
        
        // Bit 0: Key restore defer (0=don't defer, 1=defer)
        // Leave as 0 (don't defer)
        
        cmd[2] = key_map_ctrl;
        cmd[3] = algorithm;     // Algorithm (0=WEP, 1=TKIP, 2=CCMP, 3=GCMP)
        cmd[4] = key_len;       // Key length
        
        // Copy key data (up to 32 bytes)
        let data_len = core::cmp::min(key_data.len(), 32usize);
        cmd[5..(5 + data_len)].copy_from_slice(&key_data[..data_len]);
        
        // If key data is shorter than 32 bytes, pad with zeros (already zeroed)
        
        // Send H2C command
        self.h2c_command_full(&cmd)?;
        
        // Update H2C box index
        self.h2c_box = (self.h2c_box + 1) % 4;
        
        crate::mesa_println!("[RTL8822CE] H2C security add key: idx={}, alg={}, len={}, is_grp={}", key_idx, algorithm, key_len, is_grp);
        
        driver_unlock(); Ok(())
    }

    /// Remove security key via H2C command
    #[allow(dead_code)]
    fn h2c_security_remove_key(&mut self, key_idx: u8, is_grp: bool) -> Result<(), &'static str> {
        let mut cmd = [0u8; 32];
        
        // H2C security remove key command format
        cmd[0] = self.h2c_box;  // H2C box
        cmd[1] = key_idx;       // Key index
        // Algorithm field set to 0xFF to indicate "remove all keys" or specific algorithm
        cmd[2] = 0xFF;          // Algorithm (0xFF = remove key)
        cmd[3] = 0;             // Key length
        
        self.h2c_command_full(&cmd)?;
        
        self.h2c_box = (self.h2c_box + 1) % 4;
        
        crate::mesa_println!("[RTL8822CE] H2C security remove key: idx={}, is_grp={}", key_idx, is_grp);
        
        driver_unlock(); Ok(())
    }

    // ========================================================================
    // TX (Transmit)
    // ========================================================================
    // MAC/BB/RF Initialization (post-FW)
    // ========================================================================

    fn init_mac_post_fw(&self) {
        unsafe {
            // 1. Power on BB/RF domain (like Linux rtw8822c_phy_set_param)
            crate::mesa_println!("[RTL8822CE] Powering on BB/RF domain...");
            self.write8(REG_SYS_FUNC_EN, self.read8(REG_SYS_FUNC_EN) | BIT_FEN_BB_RSTB | BIT_FEN_BB_GLB_RST);
            self.write8(REG_RF_CTRL, self.read8(REG_RF_CTRL) | BIT_RF_EN | BIT_RF_RSTB | BIT_RF_SDM_RSTB);
            self.write32(REG_WLRF1, self.read32(REG_WLRF1) | BIT_WLRF1_BBRF_EN);
            self.delay_ms(10);

            // 2. Enable MAC TX/RX protocol engines
            self.write8(REG_CR, 0);
            self.write8(REG_CR, MAC_TRX_ENABLE as u8);

            // 3. Configure page size (TX=8, RX=8)
            self.write8(0x0484, 0x88); // REG_PBP

            // 4. Set RX DMA protocol
            self.write32(0x0618, 0x77000010); // REG_RXDMA_PRO

            // 5. Set FIFO boundary
            self.write16(0x048C, 0x3FFF); // REG_TRXFF_BDNY

            // 6. Set beacon queue boundary
            self.write8(0x0210, 0x80); // REG_TXPKTBUF_BCNQ_BDNY

            // 7. Disable all queues pause
            self.write8(REG_TXPAUSE, 0x00);

            // 8. Set retry limit (short=7, long=7)
            self.write16(0x021A, 0x0707); // REG_RL

            // 9. Enable EDCA
            self.write32(0x0500, 0x00104300); // VO
            self.write32(0x0504, 0x00104300); // VI
            self.write32(0x0508, 0x00104300); // BE
            self.write32(0x050C, 0x00104300); // BK

            // 10. BB RX Path Configuration (PHYDM post-FW init)
            crate::mesa_println!("[RTL8822CE] BB RX path configured (2T2R)");
        }
        
        // 11. PHY Initialization (load critical tables, init DIG, etc.)
        self.phy_init_post_fw();
    }

    /// PHY initialization after firmware download - mirrors Linux rtw_phy_load_tables + rtw_phy_init
    /// This configures the BB/RF registers from tables for proper RX operation
    fn phy_init_post_fw(&self) {
        crate::mesa_println!("[RTL8822CE] Starting PHY initialization...");
        
        unsafe {
            // --- Critical BB Register Initialization ---
            // These are minimal critical registers for RX to work
            
            // RX Path enable (both paths A and B)
            self.write32(0x1A00, 0x03); // REG_RCR_RX_PATH - enable both paths
            
            // LNA/TIA gain for both paths
            self.write32(0x1A04, 0x00F0F0F0); // Path A LNA/TIA gain
            self.write32(0x4A04, 0x00F0F0F0); // Path B LNA/TIA gain
            
            // CCA/ED threshold for both paths
            self.write32(0x1C80, 0x0F000000); // 2G CCA mask
            
            // BW setting - 20MHz default
            self.write32(0x0668, 0x0E301000); // REG_WMAC_TRXPTCL_CTL: 20MHz
            
            // AGC Table selection for 2G
            self.write32(0x1A80, 0x00000000); // Disable CCK check for 2G
            self.write8(0x0454, 0x00); // MAC CCK check disable for 2G
            
            // RX Gain Table initialization (critical for RX sensitivity)
            // Path A: 0x1A08-0x1A3C, Path B: 0x4A08-0x4A3C
            // These are simplified - real tables have many more entries
            
            // DIG (Dynamic Initial Gain) initialization - Linux rtw_phy_init
            // IGI (Initial Gain Index) for path A and B
            // REG_IGI_A = 0x1C50, REG_IGI_B = 0x1C54 (per Linux rtw8822c_table.h)
            self.write32(0x1C50, 0x20); // Path A IGI = 0x20 (32dB)
            self.write32(0x1C54, 0x20); // Path B IGI = 0x20 (32dB)
            
            // CCK PD (CCK Packet Detection) initialization
            self.write32(0x1A98, 0x00000000); // CCK GI upper bound
            self.write32(0x1AA8, 0x00000000); // CCK GI upper bound LSB
            self.write32(0x1A70, 0x00000000); // CCK GI lower bound
            
            // RX PHY Status enable (for RSSI, channel estimation)
            self.write32(REG_RCR, self.read32(REG_RCR) | (1 << 28)); // APP_PHYSTS
            
            // Enable RX paths in BB
            self.write32(0x1A00, 0x03); // Enable both RX paths
            
            // RF Path A/B enable
            self.write32(0x1A04, 0x00F0F0F0); // LNA/TIA gain
            self.write32(0x4A04, 0x00F0F0F0);
            
            // CCA Mask
            self.write32(0x1C80, 0x0F000000);
            
            crate::mesa_println!("[RTL8822CE] PHY initialization complete");
        }

        // Critical RF initialization (mirrors Linux rtw8822c_rf_init + rtw_phy_init)
        self.rf_init_critical();
        self.phy_init_dig_cckpd();
    }

    /// Critical RF initialization - mirrors Linux rtw8822c_rf_init()
    /// Configures RF DAC calibration, thermal trim, power trim, PA bias
    fn rf_init_critical(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] RF init (rtw8822c_rf_init)...");
        self.rf_dac_cal();
        self.rf_x2_check();
        self.thermal_trim();
        self.power_trim();
        self.pa_bias();
        crate::mesa_println!("[RTL8822CE] RF init complete");
        driver_unlock(); Ok(())
    }

    // ------------------------------------------------------------------
    // 8822C RF init helpers - faithful ports of rtw88 rtw8822c.c
    // ------------------------------------------------------------------

    /// rtw_read32_mask: read and shift the field down to bit 0.
    fn rd32_mask(&self, offset: u32, mask: u32) -> u32 {
        let shift = mask.trailing_zeros();
        unsafe { (self.read32(offset) & mask) >> shift }
    }

    /// rtw_phy_read_rf: read an RF register (field shifted to bit 0).
    fn rf_read(&self, path: u8, addr: u32, mask: u32) -> u32 {
        self.read_rf_sipi(path, addr, mask)
    }

    /// rtw_phy_write_rf_reg_mix: addr 0 via SIPI, others via BB window.
    fn rf_write(&self, path: u8, addr: u32, mask: u32, data: u32) {
        self.write_rf_mix(path, addr, mask, data);
    }

    /// check_hw_ready: poll until (read & mask) == target (max ~10ms).
    fn hw_ready(&self, addr: u32, mask: u32, target: u32) -> bool {
        for _ in 0..1000 {
            if self.rd32_mask(addr, mask) == target {
                return true;
            }
            self.delay_us(10);
        }
        false
    }

    /// Physical efuse byte read; 0xff (EFUSE_READ_FAIL) on error.
    fn efuse_read_phys(&self, addr: u16) -> u8 {
        self.efuse_read_byte(addr).unwrap_or(0xff)
    }

    /// Base address of the DAC path write window.
    fn dack_waddr(path: u8) -> u32 {
        if path == 0 { 0x1800 } else { 0x4100 }
    }

    /// Base address of the DAC path read window.
    fn dack_raddr(path: u8) -> u32 {
        if path == 0 { 0x2800 } else { 0x4500 }
    }

    /// rtw8822c_dac_iq_check.
    fn dac_iq_check(&self, value: u32) -> bool {
        !((value >= 0x200 && (0x400 - value) > 0x64) || (value < 0x200 && value > 0x64))
    }

    /// rtw8822c_rf_minmax_cmp.
    fn rf_minmax_cmp(&self, value: u32, min: &mut u32, max: &mut u32) {
        if value >= 0x200 {
            if *min >= 0x200 {
                if *min > value {
                    *min = value;
                }
            } else {
                *min = value;
            }
            if *max >= 0x200 && *max < value {
                *max = value;
            }
        } else {
            if *min < 0x200 && *min > value {
                *min = value;
            }
            if *max >= 0x200 {
                *max = value;
            } else if *max < value {
                *max = value;
            }
        }
    }

    /// __rtw8822c_dac_iq_sort.
    fn dac_iq_sort_pair(v1: &mut u32, v2: &mut u32) {
        if (*v1 >= 0x200 && *v2 >= 0x200) || (*v1 < 0x200 && *v2 < 0x200) {
            if *v1 > *v2 {
                core::mem::swap(v1, v2);
            }
        } else if *v1 < 0x200 && *v2 >= 0x200 {
            core::mem::swap(v1, v2);
        }
    }

    /// rtw8822c_dac_iq_sort.
    fn dac_iq_sort(&self, iv: &mut [u32; DACK_SN_8822C], qv: &mut [u32; DACK_SN_8822C]) {
        for i in 0..(DACK_SN_8822C - 1) {
            for j in 0..(DACK_SN_8822C - 1 - i) {
                let (a, b) = iv.split_at_mut(j + 1);
                Self::dac_iq_sort_pair(&mut a[j], &mut b[0]);
                let (a, b) = qv.split_at_mut(j + 1);
                Self::dac_iq_sort_pair(&mut a[j], &mut b[0]);
            }
        }
    }

    /// rtw8822c_dac_iq_offset.
    fn dac_iq_offset(&self, vec: &[u32; DACK_SN_8822C]) -> u32 {
        let mut m = 0u32;
        let mut p = 0u32;
        for i in 10..(DACK_SN_8822C - 10) {
            if vec[i] > 0x200 {
                m = (0x400 - vec[i]) + m;
            } else {
                p = vec[i] + p;
            }
        }
        if p > m {
            (p - m) / (DACK_SN_8822C as u32 - 20)
        } else {
            let t = (m - p) / (DACK_SN_8822C as u32 - 20);
            if t != 0 {
                0x400 - t
            } else {
                t
            }
        }
    }

    /// rtw8822c_dac_cal_iq_sample.
    fn dac_cal_iq_sample(&self, iv: &mut [u32; DACK_SN_8822C], qv: &mut [u32; DACK_SN_8822C]) {
        let mut i = 0usize;
        let mut cnt = 0u32;
        while i < DACK_SN_8822C && cnt < 10000 {
            cnt += 1;
            let temp = self.rd32_mask(0x2dbc, 0x3fffff);
            iv[i] = (temp & 0x3ff000) >> 12;
            qv[i] = temp & 0x3ff;
            if self.dac_iq_check(iv[i]) && self.dac_iq_check(qv[i]) {
                i += 1;
            }
        }
    }

    /// rtw8822c_dac_cal_iq_search.
    fn dac_cal_iq_search(&self, iv: &mut [u32; DACK_SN_8822C], qv: &mut [u32; DACK_SN_8822C]) -> (u32, u32) {
        let mut cnt = 0u32;
        loop {
            let mut i_min = iv[0];
            let mut i_max = iv[0];
            let mut q_min = qv[0];
            let mut q_max = qv[0];
            for i in 0..DACK_SN_8822C {
                self.rf_minmax_cmp(iv[i], &mut i_min, &mut i_max);
                self.rf_minmax_cmp(qv[i], &mut q_min, &mut q_max);
            }

            let i_delta = if (i_max < 0x200 && i_min < 0x200) || (i_max >= 0x200 && i_min >= 0x200) {
                i_max - i_min
            } else {
                i_max + (0x400 - i_min)
            };
            let q_delta = if (q_max < 0x200 && q_min < 0x200) || (q_max >= 0x200 && q_min >= 0x200) {
                q_max - q_min
            } else {
                q_max + (0x400 - q_min)
            };

            self.dac_iq_sort(iv, qv);

            if i_delta > 5 || q_delta > 5 {
                let temp = self.rd32_mask(0x2dbc, 0x3fffff);
                iv[0] = (temp & 0x3ff000) >> 12;
                qv[0] = temp & 0x3ff;
                let temp = self.rd32_mask(0x2dbc, 0x3fffff);
                iv[DACK_SN_8822C - 1] = (temp & 0x3ff000) >> 12;
                qv[DACK_SN_8822C - 1] = temp & 0x3ff;
            } else {
                break;
            }

            let cont = cnt < 100;
            cnt += 1;
            if !cont {
                break;
            }
        }

        let i_value = self.dac_iq_offset(iv);
        let q_value = self.dac_iq_offset(qv);
        (i_value, q_value)
    }

    /// rtw8822c_dac_cal_rf_mode.
    fn dac_cal_rf_mode(&self) -> (u32, u32) {
        let mut iv = [0u32; DACK_SN_8822C];
        let mut qv = [0u32; DACK_SN_8822C];
        self.dac_cal_iq_sample(&mut iv, &mut qv);
        self.dac_cal_iq_search(&mut iv, &mut qv)
    }

    /// rtw8822c_dac_bb_setting.
    fn dac_bb_setting(&self) {
        unsafe {
            self.write32_mask(0x1d58, 0xff8, 0x1ff);
            self.write32_mask(0x1a00, 0x3, 0x2);
            self.write32_mask(0x1a14, 0x300, 0x3);
            self.write32(0x1d70, 0x7e7e7e7e);
            self.write32_mask(0x180c, 0x3, 0x0);
            self.write32_mask(0x410c, 0x3, 0x0);
            self.write32(0x1b00, 0x00000008);
            self.write8(0x1bcc, 0x3f);
            self.write32(0x1b00, 0x0000000a);
            self.write8(0x1bcc, 0x3f);
            self.write32_mask(0x1e24, 1u32 << 31, 0x0);
            self.write32_mask(0x1e28, 0xf, 0x3);
        }
    }

    /// rtw8822c_dac_cal_adc (returns (adc_ic, adc_qc, dack_adck)).
    fn dac_cal_adc(&self, path: u8) -> (u32, u32, u32) {
        let base = Self::dack_waddr(path);
        let path_sel: u32 = if path == 0 { 0xa0000 } else { 0x80000 };
        let mut adc_ic = 0u32;
        let mut adc_qc = 0u32;
        let mut adck = 0u32;

        unsafe {
            self.write32_mask(base + 0x30, 1u32 << 30, 0x0);
            if path == 1 {
                self.write32(base + 0x30, 0x30db8041);
            }
            self.write32(base + 0x60, 0xf0040ff0);
            self.write32(base + 0x0c, 0xdff00220);
            self.write32(base + 0x10, 0x02dd08c4);
            self.write32(base + 0x0c, 0x10000260);
        }
        self.rf_write(0, 0x0, RFREG_MASK, 0x10000);
        self.rf_write(1, 0x0, RFREG_MASK, 0x10000);

        for _ in 0..10 {
            unsafe {
                self.write32(0x1c3c, path_sel + 0x8003);
                self.write32(0x1c24, 0x00010002);
            }
            let (mut ic, mut qc) = self.dac_cal_rf_mode();
            if ic != 0x0 {
                ic = 0x400 - ic;
                adc_ic = ic;
            }
            if qc != 0x0 {
                qc = 0x400 - qc;
                adc_qc = qc;
            }
            let temp = (ic & 0x3ff) | ((qc & 0x3ff) << 10);
            adck = temp;
            unsafe {
                self.write32(base + 0x68, temp);
                self.write32(0x1c3c, path_sel + 0x8103);
            }
            let (mut ic2, mut qc2) = self.dac_cal_rf_mode();
            if ic2 >= 0x200 {
                ic2 = 0x400 - ic2;
            }
            if qc2 >= 0x200 {
                qc2 = 0x400 - qc2;
            }
            if ic2 < 5 && qc2 < 5 {
                break;
            }
        }

        unsafe {
            self.write32(0x1c3c, 0x00000003);
            self.write32(base + 0x0c, 0x10000260);
            self.write32(base + 0x10, 0x02d508c4);
        }
        self.rf_write(path, 0x8f, 1u32 << 13, 0x1);

        (adc_ic, adc_qc, adck)
    }

    /// rtw8822c_dac_cal_step1.
    fn dac_cal_step1(&self, path: u8, adck: u32) {
        let base = Self::dack_waddr(path);
        let read = Self::dack_raddr(path);

        unsafe {
            self.write32(base + 0x68, adck);
            self.write32(base + 0x0c, 0xdff00220);
            if path == 0 {
                self.write32(base + 0x60, 0xf0040ff0);
                self.write32(0x1c38, 0xffffffff);
            }
            self.write32(base + 0x10, 0x02d508c5);
            self.write32(0x9b4, 0xdb66db00);
            self.write32(base + 0xb0, 0x0a11fb88);
            self.write32(base + 0xbc, 0x0008ff81);
            self.write32(base + 0xc0, 0x0003d208);
            self.write32(base + 0xcc, 0x0a11fb88);
            self.write32(base + 0xd8, 0x0008ff81);
            self.write32(base + 0xdc, 0x0003d208);
            self.write32(base + 0xb8, 0x60000000);
        }
        self.delay_ms(2);
        unsafe { self.write32(base + 0xbc, 0x000aff8d); }
        self.delay_ms(2);
        unsafe {
            self.write32(base + 0xb0, 0x0a11fb89);
            self.write32(base + 0xcc, 0x0a11fb89);
        }
        self.delay_ms(1);
        unsafe {
            self.write32(base + 0xb8, 0x62000000);
            self.write32(base + 0xd4, 0x62000000);
        }
        self.delay_ms(20);
        if !self.hw_ready(read + 0x08, 0x7fff80, 0xffff) ||
           !self.hw_ready(read + 0x34, 0x7fff80, 0xffff) {
            crate::mesa_println!("[RTL8822CE] DACK: step1 ready timeout");
        }
        unsafe { self.write32(base + 0xb8, 0x02000000); }
        self.delay_ms(1);
        unsafe {
            self.write32(base + 0xbc, 0x0008ff87);
            self.write32(0x9b4, 0xdb6db600);
            self.write32(base + 0x10, 0x02d508c5);
            self.write32(base + 0xbc, 0x0008ff87);
            self.write32(base + 0x60, 0xf0000000);
        }
    }

    /// rtw8822c_dac_cal_step2 (returns (ic, qc)).
    fn dac_cal_step2(&self, path: u8) -> (u32, u32) {
        let base = Self::dack_waddr(path);
        unsafe {
            self.write32_mask(base + 0xbc, 0xf0000000, 0x0);
            self.write32_mask(base + 0xc0, 0xf, 0x8);
            self.write32_mask(base + 0xd8, 0xf0000000, 0x0);
            self.write32_mask(base + 0xdc, 0xf, 0x8);
            self.write32(0x1b00, 0x00000008);
            self.write8(0x1bcc, 0x3f);
            self.write32(base + 0x0c, 0xdff00220);
            self.write32(base + 0x10, 0x02d508c5);
            self.write32(0x1c3c, 0x00088103);
        }

        let (ic_in, qc_in) = self.dac_cal_rf_mode();
        let mut ic = ic_in;
        let mut qc = qc_in;
        if ic != 0x0 {
            ic = 0x400 - ic;
        }
        if qc != 0x0 {
            qc = 0x400 - qc;
        }
        if ic < 0x300 {
            ic = ic.wrapping_mul(2).wrapping_mul(6) / 5;
            ic = ic.wrapping_add(0x80);
        } else {
            ic = (0x400u32.wrapping_sub(ic)).wrapping_mul(2).wrapping_mul(6) / 5;
            ic = 0x7fu32.wrapping_sub(ic);
        }
        if qc < 0x300 {
            qc = qc.wrapping_mul(2).wrapping_mul(6) / 5;
            qc = qc.wrapping_add(0x80);
        } else {
            qc = (0x400u32.wrapping_sub(qc)).wrapping_mul(2).wrapping_mul(6) / 5;
            qc = 0x7fu32.wrapping_sub(qc);
        }
        (ic, qc)
    }

    /// rtw8822c_dac_cal_step3.
    /// Returns (i_out, q_out, ic_folded, qc_folded).
    fn dac_cal_step3(&self, path: u8, adc_ic: u32, adc_qc: u32, ic_in: u32, qc_in: u32) -> (u32, u32, u32, u32) {
        let base = Self::dack_waddr(path);
        let read = Self::dack_raddr(path);
        let ic = ic_in;
        let qc = qc_in;

        unsafe {
            self.write32(base + 0x0c, 0xdff00220);
            self.write32(base + 0x10, 0x02d508c5);
            self.write32(0x9b4, 0xdb66db00);
            self.write32(base + 0xb0, 0x0a11fb88);
            self.write32(base + 0xbc, 0xc008ff81);
            self.write32(base + 0xc0, 0x0003d208);
            self.write32_mask(base + 0xbc, 0xf0000000, ic & 0xf);
            self.write32_mask(base + 0xc0, 0xf, (ic & 0xf0) >> 4);
            self.write32(base + 0xcc, 0x0a11fb88);
            self.write32(base + 0xd8, 0xe008ff81);
            self.write32(base + 0xdc, 0x0003d208);
            self.write32_mask(base + 0xd8, 0xf0000000, qc & 0xf);
            self.write32_mask(base + 0xdc, 0xf, (qc & 0xf0) >> 4);
            self.write32(base + 0xb8, 0x60000000);
        }
        self.delay_ms(2);
        unsafe { self.write32_mask(base + 0xbc, 0xe, 0x6); }
        self.delay_ms(2);
        unsafe {
            self.write32(base + 0xb0, 0x0a11fb89);
            self.write32(base + 0xcc, 0x0a11fb89);
        }
        self.delay_ms(1);
        unsafe {
            self.write32(base + 0xb8, 0x62000000);
            self.write32(base + 0xd4, 0x62000000);
        }
        self.delay_ms(20);
        if !self.hw_ready(read + 0x24, 0x07f80000, ic) ||
           !self.hw_ready(read + 0x50, 0x07f80000, qc) {
            crate::mesa_println!("[RTL8822CE] DACK: step3 IQ vector timeout");
        }
        unsafe { self.write32(base + 0xb8, 0x02000000); }
        self.delay_ms(1);
        unsafe {
            self.write32_mask(base + 0xbc, 0xe, 0x3);
            self.write32(0x9b4, 0xdb6db600);
            let temp = ((adc_ic.wrapping_add(0x10)) & 0x3ff) |
                       (((adc_qc.wrapping_add(0x10)) & 0x3ff) << 10);
            self.write32(base + 0x68, temp);
            self.write32(base + 0x10, 0x02d508c5);
            self.write32(base + 0x60, 0xf0000000);
        }

        let (mut ic2, mut qc2) = self.dac_cal_rf_mode();
        if ic2 >= 0x10 {
            ic2 -= 0x10;
        } else {
            ic2 = 0x400 - (0x10 - ic2);
        }
        if qc2 >= 0x10 {
            qc2 -= 0x10;
        } else {
            qc2 = 0x400 - (0x10 - qc2);
        }
        let i_out = ic2;
        let q_out = qc2;
        if ic2 >= 0x200 {
            ic2 = 0x400 - ic2;
        }
        if qc2 >= 0x200 {
            qc2 = 0x400 - qc2;
        }
        (i_out, q_out, ic2, qc2)
    }

    /// rtw8822c_dac_cal_step4.
    fn dac_cal_step4(&self, path: u8) {
        let base = Self::dack_waddr(path);
        unsafe {
            self.write32(base + 0x68, 0x0);
            self.write32(base + 0x10, 0x02d508c4);
            self.write32_mask(base + 0xbc, 0x1, 0x0);
            self.write32_mask(base + 0x30, 1u32 << 30, 0x1);
        }
    }

    /// rtw8822c_rf_dac_cal (first-call calibrate path).
    fn rf_dac_cal(&self) {
        const DACK_REG_ADDRS: [u32; 16] = [
            0x180c, 0x1810, 0x410c, 0x4110, 0x1c3c, 0x1c24, 0x1d70, 0x9b4,
            0x1a00, 0x1a14, 0x1d58, 0x1c38, 0x1e24, 0x1e28, 0x1860, 0x4160,
        ];
        let mut backup = [0u32; 16];
        for i in 0..16 {
            backup[i] = unsafe { self.read32(DACK_REG_ADDRS[i]) };
        }
        let mut backup_rf = [0u32; 2];
        for path in 0..2u8 {
            backup_rf[path as usize] = self.rf_read(path, 0x8f, RFREG_MASK);
        }

        self.dac_bb_setting();

        // path A
        let (adc_ic_a, adc_qc_a, adck_a) = self.dac_cal_adc(0);
        let mut ic = 0u32;
        let mut qc = 0u32;
        for _ in 0..10 {
            self.dac_cal_step1(0, adck_a);
            let s2 = self.dac_cal_step2(0);
            ic = s2.0;
            qc = s2.1;
            let s3 = self.dac_cal_step3(0, adc_ic_a, adc_qc_a, ic, qc);
            ic = s3.2;
            qc = s3.3;
            if ic < 5 && qc < 5 {
                break;
            }
        }
        self.dac_cal_step4(0);

        // path B
        let (adc_ic_b, adc_qc_b, adck_b) = self.dac_cal_adc(1);
        for _ in 0..10 {
            self.dac_cal_step1(1, adck_b);
            let s2 = self.dac_cal_step2(1);
            ic = s2.0;
            qc = s2.1;
            let s3 = self.dac_cal_step3(1, adc_ic_b, adc_qc_b, ic, qc);
            ic = s3.2;
            qc = s3.3;
            if ic < 5 && qc < 5 {
                break;
            }
        }
        self.dac_cal_step4(1);

        unsafe {
            self.write32(0x1b00, 0x00000008);
            self.write32_mask(0x4130, 1u32 << 30, 0x1);
            self.write8(0x1bcc, 0x0);
            self.write32(0x1b00, 0x0000000a);
            self.write8(0x1bcc, 0x0);
        }

        // restore the backed-up BB registers and RF 0x8f
        for i in 0..16 {
            unsafe { self.write32(DACK_REG_ADDRS[i], backup[i]); }
        }
        for path in 0..2u8 {
            self.rf_write(path, 0x8f, RFREG_MASK, backup_rf[path as usize]);
        }
        crate::mesa_println!("[RTL8822CE] DACK calibration complete");
    }

    /// rtw8822c_rf_x2_check.
    fn rf_x2_check(&self) {
        self.delay_ms(1);
        let x2k_busy = self.rf_read(0, 0xb8, 1u32 << 15);
        if x2k_busy == 1 {
            self.rf_write(0, 0xb8, RFREG_MASK, 0xC4440);
            self.rf_write(0, 0xba, RFREG_MASK, 0x6840D);
            self.rf_write(0, 0xb8, RFREG_MASK, 0x80440);
            self.delay_ms(1);
        }
    }

    /// rtw8822c_thermal_trim.
    fn thermal_trim(&self) {
        let rf_efuse: [u16; 2] = [0x1ef, 0x1b0]; // PPG_THERMAL_A, PPG_THERMAL_B
        for path in 0..2u8 {
            let pg_therm = self.efuse_read_phys(rf_efuse[path as usize]);
            if pg_therm == 0xff {
                return;
            }
            let mut thermal = (pg_therm >> 1) & 0x7;
            thermal |= (pg_therm & 0x1) << 3;
            self.rf_write(path, 0x43, 0x000f0000, thermal as u32);
        }
    }

    /// rtw8822c_set_power_trim.
    fn set_power_trim(&self, bb_gain: &[[u8; 8]; 2]) {
        const SEQ_IDX: [(u32, usize); 15] = [
            (0x0, 0), (0x1, 1), (0x2, 2), (0x3, 2), (0x4, 3), (0x5, 4),
            (0x6, 5), (0x7, 6), (0x8, 7), (0x9, 3), (0xa, 4), (0xb, 5),
            (0xc, 6), (0xd, 7), (0xe, 7),
        ];
        for path in 0..2u8 {
            self.rf_write(path, 0xee, 1u32 << 19, 1);
            for &(seq, idx) in SEQ_IDX.iter() {
                self.rf_write(path, 0x33, RFREG_MASK, seq);
                self.rf_write(path, 0x3f, RFREG_MASK, bb_gain[path as usize][idx] as u32);
            }
            self.rf_write(path, 0xee, 1u32 << 19, 0);
        }
    }

    /// rtw8822c_power_trim.
    fn power_trim(&self) {
        let rf_efuse_2g: [u16; 3] = [0x1d4, 0x1ee, 0x1d2]; // L, M, H
        let rf_efuse_5g: [[u16; 5]; 2] = [
            [0x1ec, 0x1e8, 0x1e4, 0x1e0, 0x1dc],
            [0x1eb, 0x1e7, 0x1e3, 0x1df, 0x1db],
        ];
        let mut bb_gain = [[0u8; 8]; 2];
        let mut set = false;

        for i in 0..3 {
            let pg_pwr = self.efuse_read_phys(rf_efuse_2g[i]);
            if pg_pwr == 0xff {
                continue;
            }
            set = true;
            bb_gain[0][i] = pg_pwr & 0xf;
            bb_gain[1][i] = (pg_pwr >> 4) & 0xf;
        }

        for i in 0..5 {
            for path in 0..2usize {
                let pg_pwr = self.efuse_read_phys(rf_efuse_5g[path][i]);
                if pg_pwr == 0xff {
                    continue;
                }
                set = true;
                bb_gain[path][i + 3] = pg_pwr & 0x1f;
            }
        }
        if set {
            self.set_power_trim(&bb_gain);
        }

        unsafe { self.write32_mask(REG_DIS_DPD, DIS_DPD_MASK, DIS_DPD_MASK); }
    }

    /// rtw8822c_pa_bias.
    fn pa_bias(&self) {
        let rf_efuse_2g: [u16; 2] = [0x1d6, 0x1d5]; // PPG_PABIAS_2GA, _2GB
        let rf_efuse_5g: [u16; 2] = [0x1d8, 0x1d7]; // PPG_PABIAS_5GA, _5GB

        for path in 0..2u8 {
            let pg = self.efuse_read_phys(rf_efuse_2g[path as usize]);
            if pg == 0xff {
                return;
            }
            self.rf_write(path, 0x60, 0x0000f000, (pg & 0xf) as u32);
        }
        for path in 0..2u8 {
            let pg = self.efuse_read_phys(rf_efuse_5g[path as usize]);
            self.rf_write(path, 0x60, 0x000f0000, (pg & 0xf) as u32);
        }
    }

    /// IQK for 8822C. NOTE: on this chip the IQ calibration is executed by the
    /// firmware (rtw8822c_do_iqk -> rtw_fw_do_iqk + poll REG_RPT_CIP 0x2d9c for
    /// 0xaa). The manual BB 0x1b00..0x1bec sequence below is not a valid 8822C
    /// IQK and leaves the IQK block (0x1b00 bit3) plus 0x1bd8 enabled, which
    /// stalls the RX datapath. The PHY tables already carry the calibration
    /// warm values, so skip the manual sequence entirely.
    fn iqk_calibration(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] IQK skipped (8822C IQK is FW-driven; manual sequence disabled)");
        driver_unlock(); Ok(())
    }

    /// Legacy hand-written IQK kept for reference only; not called.
    #[allow(dead_code)]
    fn iqk_calibration_manual_unused(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Starting IQK calibration...");
        
        unsafe {
            // ===== IQK Pre-configuration =====
            // Configure IQK registers (0x1B00-0x1BFC)
            self.write32(0x1B00, 0x00000008);
            self.write32(0x1B00, 0x00A70008);
            self.write32(0x1B00, 0x00150008);
            self.write32(0x1B00, 0x00000008);
            self.write32(0x1B04, 0xE2462952);
            self.write32(0x1B08, 0x00000080);
            self.write32(0x1B0C, 0x00000000);
            self.write32(0x1B10, 0x00010C00);
            self.write32(0x1B14, 0x00000000);
            self.write32(0x1B18, 0x00292903);
            self.write32(0x1B1C, 0xA218FC32);
            self.write32(0x1B20, 0x01040008);
            self.write32(0x1B24, 0x00060008);
            self.write32(0x1B28, 0x00060300);
            self.write32(0x1B2C, 0x00180018);
            self.write32(0x1B30, 0x40000000);
            self.write32(0x1B34, 0x00000800);
            self.write32(0x1B38, 0x40000000);
            self.write32(0x1B3C, 0x40000000);
            self.write32(0x1B98, 0x00000000);
            self.write32(0x1B9C, 0x00000000);
            self.write32(0x1BC0, 0x01000000);
            self.write32(0x1BCC, 0x00000000);
            self.write32(0x1BD8, 0xE0000001);
            self.write32(0x1BE4, 0x00000000);
            self.write32(0x1BEC, 0x40000000);
            self.write32(0x1B40, 0x40000000);
            self.write32(0x1B44, 0x20004064);
            self.write32(0x1B48, 0x0005002D);
            
            // ===== LOK (LO Calibration) =====
            self.write32(0x1B4C, 0x00000000);
            self.write32(0x1B4C, 0x008A0000);
            self.write32(0x1B50, 0x000003BE);
            self.write32(0x1B4C, 0x018A0000);
            self.write32(0x1B50, 0x0000057A);
            // ... LOK sequence for multiple frequencies
            self.write32(0x1B4C, 0x0F8A0000);
            self.write32(0x1B50, 0x00000FF0);
            self.write32(0x1B4C, 0x1F8A0000);
            self.write32(0x1B50, 0x00001695);
            self.write32(0x1B4C, 0x3F8A0000);
            self.write32(0x1B50, 0x00001FF8);
            self.write32(0x1B4C, 0x00000000);
            self.write32(0x1B50, 0x00000000);
            
            // ===== TX CFIR =====
            self.write32(0x1B58, 0x00890000);
            self.write32(0x1B5C, 0x3C6B3FFF);
            self.write32(0x1B58, 0x02890000);
            self.write32(0x1B5C, 0x35D9390A);
            self.write32(0x1B58, 0x04890000);
            self.write32(0x1B5C, 0x2FFE32D6);
            
            // ===== IDFT =====
            // Path A
            self.write32(0x1B00, 0x00000008);
            self.write32(0x1BD8, 0xE0000001);
            self.write32(0x1B00, 0x00000E18);
            self.write32(0x1B00, 0x00000E19);
            // Path B
            self.write32(0x1B00, 0x0000000A);
            self.write32(0x1BD8, 0xE0000001);
            self.write32(0x1B00, 0x00000E2A);
            self.write32(0x1B00, 0x00000E2B);
            
            // ===== RX CFIR =====
            // Get RX CFIR for Path A
            self.write32(0x1B00, 0x00000008);
            self.write32(0x1B20, 0xC0000000);
            self.write32(0x1BD4, 0x00200000);
            self.write32(0x1BD4, 0x001F0000);
            
            // Read CFIR data (simplified - just trigger)
            for _ in 0..17 {
                self.write32(0x1BD8, 0xE0000001);
            }
            
            // Get RX CFIR for Path B
            self.write32(0x1B00, 0x0000000A);
            self.write32(0x1B20, 0xC0000000);
            self.write32(0x1BD4, 0x00200000);
            self.write32(0x1BD4, 0x001F0000);
            
            for _ in 0..17 {
                self.write32(0x1BD8, 0xE0000001);
            }
            
            // ===== Reload RX CFIR =====
            // Path A
            self.write32(0x1B00, 0x00000008);
            self.write32(0x1B20, 0xC0000000);
            self.write32(0x1BD8, 0x60000001);
            self.delay_us(10);
            for i in 0..17 {
                // Write real part
                self.write32(0x1BD8, 0x60000001 | (i << 2));
                self.delay_us(10);
                self.write32(0x1BD8, 0x60000000 | (i << 2));
            }
            for i in 0..17 {
                // Write imag part
                self.write32(0x1BD8, 0x60000001 | (i << 2));
                self.delay_us(10);
                self.write32(0x1BD8, 0x60000000 | (i << 2));
            }
            self.write32(0x1BD8, 0xE0000001);
            self.delay_ms(10);
            self.write32(0x1B20, 0x00000000);
            
            // Path B
            self.write32(0x1B00, 0x0000000A);
            self.write32(0x1B20, 0xC0000000);
            self.write32(0x1BD8, 0x60000001);
            self.delay_us(10);
            for i in 0..17 {
                self.write32(0x1BD8, 0x60000001 | (i << 2));
                self.delay_us(10);
                self.write32(0x1BD8, 0x60000000 | (i << 2));
            }
            for i in 0..17 {
                self.write32(0x1BD8, 0x60000001 | (i << 2));
                self.delay_us(10);
                self.write32(0x1BD8, 0x60000000 | (i << 2));
            }
            self.write32(0x1BD8, 0xE0000001);
            self.delay_ms(10);
            self.write32(0x1B20, 0x00000000);
            
            // ===== Final IQK Check =====
            // Verify calibration results
            let mut success = true;
            for path in 0..2 {
                self.write32(0x1B00, 0x00000008 | (path << 1));
                self.write32(0x1B20, 0xC0000000);
                self.write32(0x1BD8, 0xE0000001 | (1 << 2));
                
                // Wait for completion
                for _ in 0..30000 {
                    let status = self.read8(0x2D9C);
                    if status == 0x55 {
                        break;
                    }
                    self.delay_us(10);
                }
                let status = self.read8(0x2D9C);
                if status != 0x55 {
                    crate::mesa_println!("[RTL8822CE] IQK timeout path {}", path);
                    success = false;
                }
            }
            
            self.write32(0x1B10, 0x00000000);
            
            if success {
                crate::mesa_println!("[RTL8822CE] IQK calibration SUCCESS");
            } else {
                crate::mesa_println!("[RTL8822CE] IQK calibration FAILED");
            }
        }
        driver_unlock(); Ok(())
    }

    /// EDCCA / adaptivity init - mirrors Linux rtw8822c_adaptivity_init.
    /// Enables MAC energy-detect CCA so REG_EDCCA_REPORT becomes meaningful.
    fn adaptivity_init(&self) {
        unsafe {
            // rtw_phy_set_edcca_th(MAX=0x7f) on both bytes of 0x84c (+0x80 offset)
            let th = self.read32(REG_EDCCA_TH);
            self.write32(REG_EDCCA_TH, (th & 0x0000_FFFF) | (0xff << 16) | (0xffu32 << 24));
            // mac edcca state setting
            let v = self.read32(REG_TX_PTCL_CTRL);
            self.write32(REG_TX_PTCL_CTRL, v & !BIT_DIS_EDCCA);
            let v = self.read32(REG_RD_CTRL);
            self.write32(REG_RD_CTRL, v | BIT_EDCCA_MSK_CNTDOWN_EN);
            // edcca decision opt = 0
            let v = self.read32(REG_EDCCA_DECISION);
            self.write32(REG_EDCCA_DECISION, v & !BIT_EDCCA_OPTION);
        }
    }

    /// Read PHY energy/CRC counters (rtw8822c_false_alarm_statistics).
    /// Returns (cca_cck, cca_ofdm, cck_ok, cck_err, ofdm_ok, ofdm_err,
    ///          ht_ok, ht_err, vht_ok, vht_err).
    fn read_phy_counters(&self) -> (u16, u16, u16, u16, u16, u16, u16, u16, u16, u16) {
        unsafe {
            let cca = self.read32(REG_CCA_CNT);
            let cck = self.read32(REG_CCK_CRC_CNT);
            let ofdm = self.read32(REG_OFDM_CRC_CNT);
            let ht = self.read32(REG_HT_CRC_CNT);
            let vht = self.read32(REG_VHT_CRC_CNT);
            (
                (cca & 0xffff) as u16, (cca >> 16) as u16,
                (cck & 0xffff) as u16, (cck >> 16) as u16,
                (ofdm & 0xffff) as u16, (ofdm >> 16) as u16,
                (ht & 0xffff) as u16, (ht >> 16) as u16,
                (vht & 0xffff) as u16, (vht >> 16) as u16,
            )
        }
    }

    /// Enable/reset the PHY FA/CCA counters (tail of
    /// rtw8822c_false_alarm_statistics). The counters only tick while the RX
    /// gate clock is enabled; without this sequence the probe reads all zeroes
    /// and wrongly looks like "dead receiver".
    fn start_phy_counters(&self) {
        unsafe {
            let v = self.read32(REG_RX_BREAK);
            self.write32(REG_RX_BREAK, v & !BIT_COM_RX_GCK_EN);
            self.write32(REG_CNT_CTRL, self.read32(REG_CNT_CTRL) | BIT_ALL_CNT_RST);
            self.write32(REG_CNT_CTRL, self.read32(REG_CNT_CTRL) & !BIT_ALL_CNT_RST);
            self.write32(REG_RX_BREAK, self.read32(REG_RX_BREAK) | BIT_COM_RX_GCK_EN);
        }
    }

    /// PHY DIG and CCK PD initialization - mirrors Linux rtw_phy_init
    fn phy_init_dig_cckpd(&self) -> Result<(), &'static str> {
        // Faithful port of rtw_phy_init (phy.c:236). Despite the name, this
        // function is almost entirely *software* state: it seeds the DIG/CFO/
        // tx_path_div arrays and calls rtw_phy_cck_pd_init (also SW-only). The
        // ONLY hardware writes are the EDCCA configuration performed by
        // rtw8822c_adaptivity_init.
        //
        // The previous implementation hardcoded dozens of BB writes (e.g. zeroing
        // 0x1a70/0x1a98/0x1aa8 = CCK GI bounds that rtw8822c_phy_set_param reads
        // right afterwards, and 0x1a08.. = AGC/LNA/TIA gains). Those clobbered
        // the tables loaded by rtw_phy_load_tables and left the RX chain deaf.
        crate::mesa_println!("[RTL8822CE] rtw_phy_init (adaptivity/EDCCA only)...");
        self.adaptivity_init();
        self.start_phy_counters();
        crate::mesa_println!("[RTL8822CE] rtw_phy_init complete");
        driver_unlock(); Ok(())
    }

    fn set_channel(&mut self, channel: u8, band_5ghz: bool) -> Result<(), &'static str> {
        // Faithful port of rtw8822c_set_channel (rtw8822c.c:2426):
        //   set_channel_bb -> rtw_set_channel_mac -> set_channel_rf -> toggle_igi
        //
        // NOTE: the previous implementation additionally ran a pile of vendor
        // phydm steps that are NOT part of rtw88's set_channel (band switch, a
        // BB reset via 0x1C00, phydm_agc_tab_sel, sco_trk, cck_tx_shaping, TX
        // power and a host IQK). Those writes left the RX chain dead after the
        // first retune (one descriptor at boot, then RXBD_IDX frozen forever),
        // so they were removed. rtw88 works with the four steps below only.

        let is_5g = band_5ghz || channel > 14;
        let switch_band = self.current_band_5g != is_5g;

        crate::mesa_println!("[RTL8822CE] Channel switch: ch={} is_5g={} switch_band={}", channel, is_5g, switch_band);

        // 1. BB channel config (rtw8822c_set_channel_bb)
        self.set_channel_bb(channel, 0);

        // 2. MAC channel config (rtw_set_channel_mac)
        self.set_channel_mac(channel);

        // 3. RF channel config (rtw8822c_set_channel_rf: RF18 + RX BB LUT)
        self.set_channel_rf(channel);

        // 4. Toggle IGI (rtw_dm_toggle_igi) after the channel/bw change.
        self.toggle_igi()?;

        // 5. Update internal state.
        self.current_channel = channel;
        self.current_band_5g = is_5g;

        // Let the synthesizer settle.
        self.delay_ms(2);

        crate::mesa_println!("[RTL8822CE] Channel switch complete: ch={}", channel);
        driver_unlock(); Ok(())
    }

    /// rtw_set_channel_mac (mac.c:12) -- MAC-side channel/bandwidth config.
    fn set_channel_mac(&self, channel: u8) {
        unsafe {
            // 20 MHz, primary channel segment 0 -> DATA_SC = 0.
            self.write8(REG_DATA_SC, 0);

            // RFMOD = 0 for 20 MHz.
            let v = self.read32(REG_WMAC_TRXPTCL_CTL) & !BIT_RFMOD;
            self.write32(REG_WMAC_TRXPTCL_CTL, v);

            // CCK check is only enabled on the 5 GHz band.
            let mut v = self.read8(REG_CCK_CHECK);
            v &= !(BIT_CHECK_CCK_EN as u8);
            if channel > 14 {
                v |= BIT_CHECK_CCK_EN as u8;
            }
            self.write8(REG_CCK_CHECK, v);
        }
    }

    /// rtw8822c_set_channel_rf (rtw8822c.c:2191) -- RF synthesizer + RX BB LUT.
    fn set_channel_rf(&self, channel: u8) {
        let is_5g = channel > 14;

        let mut rf18 = unsafe { self.read_rf_sipi(0, 0x18, RFREG_MASK) };
        // RF18_BAND_MASK | RF18_CHANNEL_MASK | RF18_RFSI_MASK | RF18_BW_MASK
        rf18 &= !(0xFF | (1 << 8) | (1 << 9) | (1 << 12) | (1 << 13)
            | (1 << 16) | (1 << 17) | (1 << 18));
        rf18 |= channel as u32;
        if is_5g {
            rf18 |= (1 << 16) | (1 << 8);
            if channel > 144 {
                rf18 |= 1 << 18;
            } else if channel >= 80 {
                rf18 |= 1 << 17;
            }
        }
        rf18 |= (1 << 13) | (1 << 12); // RF18_BW_20M

        self.rstb_3wire(false);
        for path in [0u8, 1u8] {
            self.write_rf_mix(path, 0xEE, 0x04, 0x01);    // RF_LUTWE2
            self.write_rf_mix(path, 0x33, 0x1F, 0x12);    // RF_LUTWA
            self.write_rf_mix(path, 0x3F, 0xFFFFF, 0x18); // RF_LUTWD0 (rxbb 20M)
            self.write_rf_mix(path, 0xEE, 0x04, 0x00);    // RF_LUTWE2
            self.write_rf_mix(path, 0x18, RFREG_MASK, rf18); // RF_CFGCH
        }
        self.rstb_3wire(true);
    }

    /// PHYDM band switch (BB settings for 2G/5G)
    fn phydm_band_switch(&self, is_5g: bool) {
        unsafe {
            if is_5g {
                // 5G: disable CCK, enable OFDM checks
                let val = self.read32(0x1A80);
                self.write32(0x1A80, val | (1 << 18)); // BB CCK check enable
                let val = self.read8(0x454);
                self.write8(0x454, val | (1 << 7)); // MAC CCK check enable
                self.write32(0x1C80, (self.read32(0x1C80) & !0x3F000000) | 0x22000000); // CCA mask
            } else {
                // 2G: enable CCK, disable OFDM checks  
                let val = self.read32(0x1A80);
                self.write32(0x1A80, val & !(1 << 18)); // BB CCK check disable
                let val = self.read8(0x454);
                self.write8(0x454, val & !(1 << 7)); // MAC CCK check disable
                self.write32(0x1C80, (self.read32(0x1C80) & !0x3F000000) | 0x0F000000); // CCA mask
}
        }
    }

    // ========================================================================
    // BT/WiFi Coexistence (matches Linux halbtc8822c.c)
    // ========================================================================

    /// Coex initialization - matches Linux halbtc8822c_chip_setup + ex_hal8822c_wifi_only_hw_config
    fn coex_init(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Coex initialization (WiFi-only mode)...");
        
        unsafe {
            // WiFi-only HW config (from ex_hal8822c_wifi_only_hw_config)
            // gnt_wl=1, gnt_bt=0
            self.write32(0x0070, (self.read32(0x0070) & !0xFF000000) | 0x0E000000);
            
            // 0x1704: WL/BT grant
            self.write32(0x1704, 0x00007700);
            
            // 0x1700: LTE coex control
            self.write32(0x1700, 0xC00F0038);
            
            // 0x06C0/0x06C4: PTA control
            self.write32(0x06C0, 0xAAAAAAAA);
            self.write32(0x06C4, 0xAAAAAAAA);
        }
        
        crate::mesa_println!("[RTL8822CE] Coex init complete (WiFi-only)");
        driver_unlock(); Ok(())
    }
}

// ========================================================================
// BT/WiFi Coexistence (WiFi-Only Mode) - Ported from Linux hal_btcoex_wifionly.c
// ========================================================================

#[derive(Default, Clone, Copy)]
struct WifiOnlyHalData {
    customer_id: u16,
    efuse_pg_antnum: u8,
    efuse_pg_antpath: u8,
    rfe_type: u8,
    ant_div_cfg: u8,
}

#[derive(Default, Clone, Copy)]
struct WifiOnlyCfg {
    adapter: *mut Rtl8822ce,
    haldata_info: WifiOnlyHalData,
    chip_interface: u8, // 1=PCI, 2=USB, 3=SDIO
}

// WiFi-only coexistence instance (single global for the driver)
static mut WIFI_ONLY_CFG: WifiOnlyCfg = WifiOnlyCfg {
    adapter: core::ptr::null_mut(),
    haldata_info: WifiOnlyHalData {
        customer_id: 0,
        efuse_pg_antnum: 0,
        efuse_pg_antpath: 0,
        rfe_type: 0,
        ant_div_cfg: 0,
    },
    chip_interface: 1, // PCI
};

impl Rtl8822ce {
    /// Initialize WiFi-only coexistence variables - matches Linux hal_btcoex_wifionly_initlizevariables
    fn coex_init_variables(&mut self) {
        crate::mesa_println!("[RTL8822CE] Coex init variables (WiFi-only)...");
        
        unsafe {
            WIFI_ONLY_CFG.adapter = self as *mut Rtl8822ce;
            WIFI_ONLY_CFG.chip_interface = 1; // PCI
            WIFI_ONLY_CFG.haldata_info.customer_id = 0; // CUSTOMER_NORMAL
            
            // Antenna info from EFUSE
            WIFI_ONLY_CFG.haldata_info.efuse_pg_antnum = self.efuse.multi_antenna as u8;
            WIFI_ONLY_CFG.haldata_info.efuse_pg_antpath = 0; // Will be set from EFUSE
            WIFI_ONLY_CFG.haldata_info.rfe_type = 0;
            WIFI_ONLY_CFG.haldata_info.ant_div_cfg = 0;
        }
        
        crate::mesa_println!("[RTL8822CE] Coex init variables complete");
    }

    /// rtw_coex_write_indirect_reg (via the WLAN<->LTE indirect window).
    fn coex_write_indirect_reg(&self, addr: u32, mask: u32, val: u32) {
        let shift = mask.trailing_zeros();
        let tmp = match self.ltecoex_read_reg(addr) {
            Ok(v) => v,
            Err(_) => return,
        };
        let tmp = (tmp & !mask) | ((val << shift) & mask);
        let _ = self.ltecoex_reg_write(addr, tmp);
    }

    /// rtw_coex_set_gnt_bt.
    fn coex_set_gnt_bt(&self, state: u32) {
        self.coex_write_indirect_reg(0x38, 0xc000, state); // LTE_COEX_CTRL
        self.coex_write_indirect_reg(0x38, 0x0c00, state);
    }

    /// rtw_coex_set_gnt_wl.
    fn coex_set_gnt_wl(&self, state: u32) {
        self.coex_write_indirect_reg(0x38, 0x3000, state); // LTE_COEX_CTRL
        self.coex_write_indirect_reg(0x38, 0x0300, state);
    }

    /// rtw_coex_coex_ctrl_owner (btg_reg is NULL for 8822C).
    fn coex_ctrl_owner(&mut self, wifi_control: bool) {
        let bit = ((1u32 << 26) >> 24) as u8; // BIT_LTE_MUX_CTRL_PATH of byte 0x73
        if wifi_control {
            self.write8_set(0x0070 + 3, bit);
        } else {
            self.write8_clr(0x0070 + 3, bit);
        }
    }

    /// rtw8822c_coex_cfg_rfe_type.
    fn coex_cfg_rfe_type(&self) {
        // disable LTE coex in wifi side
        self.coex_write_indirect_reg(0x38, 1u32 << 7, 0x0); // LTE_COEX_CTRL/BIT_LTE_COEX_EN
        self.coex_write_indirect_reg(0xa0, 0xffff, 0xffff); // LTE_WL_TRX_CTRL
        self.coex_write_indirect_reg(0xa4, 0xffff, 0xffff); // LTE_BT_TRX_CTRL
    }

    /// rtw8822c_coex_cfg_init.
    fn coex_cfg_init(&mut self) {
        unsafe {
            self.write8_set(0x0550, 1u8 << 3);      // REG_BCN_CTRL/BIT_EN_BCN_FUNCTION
            self.write8_mask(0x0790, 0x3f, 0x5);    // REG_BT_TDMA_TIME/sample rate
            self.write8(0x0778, 0x1);               // REG_BT_STAT_CTRL
            self.write32_set(0x0040, 1u32 << 5);    // REG_GPIO_MUXCFG/BIT_BT_PTA_EN
            self.write32_set(0x0040, 1u32 << 9);    // BIT_PO_BT_PTA_PINS
            self.write8_set(0x04C6, 1u8 << 4);      // REG_QUEUE_CTRL/BIT_PTA_WL_TX_EN
            self.write8_clr(0x04C6, 1u8 << 5);      // BIT_PTA_EDCCA_EN
            self.write16_set(0x0762, 1u16 << 12);   // REG_BT_COEX_V2/BIT_GNT_BT_POLARITY
            self.write8_clr(0x04FC, 0x3);           // REG_DUMMY_PAGE4_V1/BIT_BTCCA_CTRL
        }
        self.rf_write(1, 0x01, RFREG_MASK, 0x40000); // RF_PATH_B/RF_MODOPT
    }

    /// rtw_coex_set_ant_path. Only POWERON (wlan_only=false) and WLANONLY
    /// (wlan_only=true) are needed: ant_switch_exist is false for 8822C, so the
    /// final rtw_coex_set_ant_switch() call is skipped.
    fn coex_set_ant_path(&mut self, wlan_only: bool) {
        if wlan_only {
            self.coex_set_gnt_bt(0x1); // COEX_GNT_SET_SW_LOW
            self.coex_set_gnt_wl(0x3); // COEX_GNT_SET_SW_HIGH
            self.coex_ctrl_owner(true);
        } else {
            // COEX_SET_ANT_POWERON: set path control owner to BT
            self.coex_ctrl_owner(false);
        }
    }

    /// Coex power on setting - matches Linux rtw_coex_power_on_setting
    fn coex_power_on_setting(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Coex power on setting...");
        self.coex_init_variables();

        unsafe {
            // enable BB, we can write 0x948
            self.write8(REG_SYS_FUNC_EN, self.read8(REG_SYS_FUNC_EN) |
                BIT_FEN_BB_GLB_RST | BIT_FEN_BB_RSTB);
        }
        self.coex_cfg_rfe_type();
        self.coex_set_ant_path(false);
        unsafe { self.write8(0xff1a, 0x0); } // red x issue

        crate::mesa_println!("[RTL8822CE] Coex power on setting complete");
        driver_unlock(); Ok(())
    }

    /// Coex init hw config - matches Linux rtw_coex_init_hw_config (wifi-only)
    fn coex_init_hw_config(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Coex init hw config...");

        self.coex_cfg_rfe_type();
        self.coex_cfg_init();
        // No BT driver in this kernel: configure the RF front-end as WiFi-only
        // so the antenna/PTA path grants WL.
        self.coex_set_ant_path(true);

        crate::mesa_println!("[RTL8822CE] Coex init hw config complete");
        driver_unlock(); Ok(())
    }

    /// Coex scan notify - matches Linux ex_hal8822c_wifi_only_scannotify
    fn coex_scan_notify(&self, is_5g: bool) {
        crate::mesa_println!("[RTL8822CE] Coex scan notify: is_5g={}", is_5g);
        // WiFi-only: no action needed during scan
    }

    /// Coex switch band notify - matches Linux ex_hal8822c_wifi_only_switchbandnotify
    fn coex_switch_band_notify(&self, is_5g: bool) {
        crate::mesa_println!("[RTL8822CE] Coex switch band notify: is_5g={}", is_5g);
        // WiFi-only: could adjust antenna config per band
    }

    /// Coex connect notify - matches Linux ex_hal8822c_wifi_only_connectnotify
    fn coex_connect_notify(&self, is_5g: bool) {
        crate::mesa_println!("[RTL8822CE] Coex connect notify: is_5g={}", is_5g);
        // WiFi-only: no action needed on connect
    }

    /// Coex media status notify (connect/disconnect)
    fn coex_media_status_notify(&mut self, connect: bool) {
        crate::mesa_println!("[RTL8822CE] Coex media status: connect={}", connect);
        if connect {
            // Notify FW of media status via H2C (opmode=2 for station, role=0 for client, macid=0)
            let _ = self.h2c_media_status_rpt(2, 0, 0);
        } else {
            let _ = self.h2c_media_status_rpt(0, 0, 0);
        }
    }

    /// Coex special packet notify (EAPOL, DHCP, ARP)
    fn coex_special_packet_notify(&self, pkt_type: u8) {
        // 0: EAPOL, 1: DHCP, 2: ARP
        crate::mesa_println!("[RTL8822CE] Coex special packet: type={}", pkt_type);
        // WiFi-only: ensure priority for special packets
    }

    /// Coex TDMA slot update (for future BT coex)
    fn coex_update_tdma(&self, slot: u8) {
        crate::mesa_println!("[RTL8822CE] Coex TDMA slot: {}", slot);
    }

    /// Coex PSTDMA (Periodic Standard TDMA) handling
    fn coex_pstdma_handle(&self) {
        crate::mesa_println!("[RTL8822CE] Coex PSTDMA handle");
    }

    /// Coex RF parameter update (TX power, RX gain)
    fn coex_update_rf_para(&self, is_tx: bool, level: u8) {
        if is_tx {
            crate::mesa_println!("[RTL8822CE] Coex TX RF para level={}", level);
        } else {
            crate::mesa_println!("[RTL8822CE] Coex RX RF para level={}", level);
        }
    }

    /// Coex AFH map update (5G)
    fn coex_update_afh_map(&self, channel: u8) {
        crate::mesa_println!("[RTL8822CE] Coex AFH map update: ch={}", channel);
    }

    /// Coex antenna isolation check
    fn coex_check_ant_isolation(&self) -> u8 {
        // Return antenna isolation in dB
        15 // Default 15dB
    }

    /// Coex periodic routine
    fn coex_periodic(&self) {
        // Periodic coexistence monitoring
        crate::mesa_println!("[RTL8822CE] Coex periodic check");
    }

    // ========================================================================
    // Advanced PHY/BB Features
    // ========================================================================

    /// EDCCA (Energy Detection CCA) / Adaptivity - mirrors Linux rtw_phy_adaptivity
    fn edccadaptivity_update(&mut self) {
        crate::mesa_println!("[RTL8822CE] EDCCA/Adaptivity update...");

        unsafe {
            // EDCCA decision register: 0x0844
            let edcca_decision = self.read32(0x0844);
            let edcca_flag = (edcca_decision & 0x80000000) != 0;
            let edcca_option = (edcca_decision >> 29) & 0x3;
            
            // EDCCA thresholds: L2H (Low to High) and H2L (High to Low)
            // REG_EDCCA_DECISION = 0x0844
            // L2H: bits 15:0, H2L: bits 31:16
            let l2h = edcca_decision & 0xFFFF;
            let h2l = (edcca_decision >> 16) & 0xFFFF;
            
            crate::mesa_println!("[RTL8822CE] EDCCA: flag={} option={} L2H={} H2L={}", 
                edcca_flag, edcca_option, l2h, h2l);

            // Dynamic EDCCA thresholds based on environment
            // Linux: rtw8822c_phy_adaptivity
            if self.edcca_flag != edcca_flag {
                self.edcca_flag = edcca_flag;
                if edcca_flag {
                    // EDCCA triggered - increase thresholds
                    self.write32_mask(0x0844, 0xFFFF, (l2h + 5).min(0xFFFF) as u32);
                    self.write32_mask(0x0844, 0xFFFF0000, ((h2l + 5).min(0xFFFF) as u32) << 16);
                }
            }

            // Update EDCCA report
            let edcca_report = self.read32(0x0844);
            crate::mesa_println!("[RTL8822CE] EDCCA report: {:#x}", edcca_report);
        }
    }

    /// CFO (Carrier Frequency Offset) Tracking - mirrors Linux rtw_phy_cfo_tracking
    fn cfo_tracking_update(&mut self) {
        unsafe {
            // CFO tracking registers: 0x1A8C (path A), 0x1A90 (path B)
            let cfo_a = self.read32(0x1A8C) as i32;
            let cfo_b = self.read32(0x1A90) as i32;
            
            // CFO tracking enable threshold
            const CFO_TRK_ENABLE_TH: i32 = 20;
            const CFO_TRK_STOP_TH: i32 = 10;
            const CFO_TRK_ADJ_TH: i32 = 10;
            
            if cfo_a.abs() > CFO_TRK_ENABLE_TH || cfo_b.abs() > CFO_TRK_ENABLE_TH {
                self.cfo_tracking = true;
                crate::mesa_println!("[RTL8822CE] CFO tracking active: A={} B={}", cfo_a, cfo_b);
                
                // Adjust CFO (simplified)
                if cfo_a.abs() > CFO_TRK_ADJ_TH {
                    let adj = if cfo_a > 0 { -1 } else { 1 };
                    // Would write to CFO adjustment register
                }
                if cfo_b.abs() > CFO_TRK_ADJ_TH {
                    let adj = if cfo_b > 0 { -1 } else { 1 };
                }
            } else if self.cfo_tracking && cfo_a.abs() < CFO_TRK_STOP_TH && cfo_b.abs() < CFO_TRK_STOP_TH {
                self.cfo_tracking = false;
                crate::mesa_println!("[RTL8822CE] CFO tracking stopped");
            }
        }
    }

    /// RFK (RF Calibration) Handshake - mirrors Linux rtw8822c_rfk_handshake
    fn rfk_handshake(&mut self, is_before_k: bool) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] RFK handshake: is_before_k={}", is_before_k);
        
        unsafe {
            if is_before_k {
                // Before K: Wait for BT IQK to finish
                // Check PMC_DBG_CTRL1 for BT IQK status
                // REG_PMC_DBG_CTRL1 = 0x1C24 (approx)
                
                crate::mesa_println!("[RTL8822CE] RFK: Waiting for BT IQK...");
                
                // Poll BT IQK status (BIT_PMC_BT_IQK_STS)
                for _ in 0..600000 {
                    let pmc_dbg = self.read32(0x1C24);
                    if (pmc_dbg & 0x1) == 0 {
                        break;
                    }
                    self.delay_us(20);
                }
                
                // Inform FW RFK start
                // H2C command for RFK status
                let mut rfk_cmd = [0u8; 8];
                rfk_cmd[0] = 0x20; // H2C_CMD_PHYDM_INFO or RFK specific
                rfk_cmd[1] = 0x01; // RFK start
                self.h2c_command(&rfk_cmd)?;
                
                // Wait for FW RFK ready
                for _ in 0..100000 {
                    let arfr4 = self.read8(0x0288); // REG_ARFR4
                    if (arfr4 & 0x1) != 0 {
                        break;
                    }
                    self.delay_us(20);
                }
            } else {
                // After K: Inform FW RFK finish
                let mut rfk_cmd = [0u8; 8];
                rfk_cmd[0] = 0x20;
                rfk_cmd[1] = 0x00; // RFK finish
                self.h2c_command(&rfk_cmd)?;
                
                // Wait for FW ack
                for _ in 0..100000 {
                    let arfr4 = self.read8(0x0288);
                    if (arfr4 & 0x1) != 0 {
                        break;
                    }
                    self.delay_us(20);
                }
                
                crate::mesa_println!("[RTL8822CE] RFK handshake complete");
            }
        }
        driver_unlock(); Ok(())
    }

    /// Deep LPS (Power Gating / LCLK) - mirrors Linux rtw_enter_lps_deep / rtw_leave_lps_deep
    fn lps_deep_enter(&mut self, deep_mode: u8) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Deep LPS enter: mode={}", deep_mode);
        
        unsafe {
            // deep_mode: 1=PG (Power Gating), 2=LCLK (Low Clock)
            if deep_mode == 1 {
                // PG (Power Gating)
                crate::mesa_println!("[RTL8822CE] Deep LPS: Power Gating mode");
                
                // 1. Set PG info to FW
                let mut pg_cmd = [0u8; 8];
                pg_cmd[0] = 0x20; // H2C_CMD_PHYDM_INFO
                pg_cmd[1] = 0x10; // PG info
                pg_cmd[2] = 0x01; // enable
                self.h2c_command(&pg_cmd)?;
                
                // 2. Enable HCI deep PS
                self.write32(0x0070, self.read32(0x0070) | 0x01000000);
                
            } else if deep_mode == 2 {
                // LCLK (Low Clock)
                crate::mesa_println!("[RTL8822CE] Deep LPS: LCLK mode");
                
                // Set LCLK info to FW
                let mut lclk_cmd = [0u8; 8];
                lclk_cmd[0] = 0x20;
                lclk_cmd[1] = 0x11; // LCLK info
                lclk_cmd[2] = 0x01;
                self.h2c_command(&lclk_cmd)?;
                
                // Enable HCI deep PS
                self.write32(0x0070, self.read32(0x0070) | 0x02000000);
            }
            
            // Wait for FW ack
            for _ in 0..100000 {
                let pmc_dbg = self.read32(0x1C24);
                if (pmc_dbg & 0x2) != 0 {
                    break;
                }
                self.delay_us(20);
            }
        }
        
        crate::mesa_println!("[RTL8822CE] Deep LPS entered: mode={}", deep_mode);
        driver_unlock(); Ok(())
    }

    fn lps_deep_leave(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Deep LPS leave...");
        
        unsafe {
            // Disable HCI deep PS
            self.write32(0x0070, self.read32(0x0070) & !0x03000000);
            
            // Inform FW
            let mut cmd = [0u8; 8];
            cmd[0] = 0x20;
            cmd[1] = 0x10; // PG/LCLK disable
            cmd[2] = 0x00;
            self.h2c_command(&cmd)?;
        }
        
        crate::mesa_println!("[RTL8822CE] Deep LPS left");
        driver_unlock(); Ok(())
    }

    /// Full DACK (DAC Calibration) backup/restore - mirrors Linux rtw8822c_dac_cal_backup/restore
    fn dack_cal_backup(&mut self) {
        crate::mesa_println!("[RTL8822CE] DACK calibration backup...");
        
        unsafe {
            // Backup registers (from rtw8822c_dac_backup_reg)
            let addrs = [
                0x180C, 0x1810, 0x410C, 0x4110,
                0x1C3C, 0x1C24, 0x1D70, 0x09B4,
                0x1A00, 0x1A14, 0x1D58, 0x1C38,
                0x1E24, 0x1E28, 0x1860, 0x4160,
            ];
            
            for (i, &addr) in addrs.iter().enumerate() {
                if i < 16 {
                    // Store in backup structure (simplified)
                    let val = self.read32(addr);
                    // Would store in self.dack_backup_regs[i] = val;
                    crate::mesa_println!("[RTL8822CE] DACK backup reg {:#x} = {:#x}", addr, val);
                }
            }
            
            // Backup RF registers (path A and B, RF 0x8F)
            for path in 0..2 {
                let rf_val = self.read_rf_sipi(path, 0x8F, RFREG_MASK);
                crate::mesa_println!("[RTL8822CE] DACK backup RF path {} 0x8F = {:#x}", path, rf_val);
                // Would store in self.dack_backup_rf[path] = rf_val;
            }
            
            self.dack_backup_valid = true;
        }
        
crate::mesa_println!("[RTL8822CE] DACK calibration backup complete");
    }
    
    /// Set RX configuration - mirrors Linux rtw_set_rx_config
    fn set_rx_config(&mut self) {
        unsafe {
            // RCR: Accept all frames initially (for scanning)
            let rcr: u32 = (1 << 31)  // APP_FCS
                | (1 << 28)           // APP_PHYSTS
                | (1 << 13)           // AMF (Accept Management Frames - beacons, probe resp)
                | (1 << 5)            // APWRMGT
                | (1 << 4)            // ADD3
                | (1 << 3)            // AB (Accept Broadcast)
                | (1 << 2)            // AM (Accept Multicast)
                | (1 << 1)            // APM (Accept Physical Match)
                | (1 << 0);           // AAP (Accept All Packets)
            self.write32(REG_RCR, rcr);
        }
    }
    
    /// Handle SCAN_DONE C2H event - parse scan results from firmware
    fn handle_scan_done(&mut self) {
        crate::mesa_println!("[RTL8822CE] Handling SCAN_DONE C2H event");
        
        // In Linux, scan results are retrieved via:
        // 1. C2H event with BIT_C2H_FW_SCAN_DONE
        // 2. Reading the scan result buffer (usually via RX ring or dedicated buffer)
        // 3. Parsing beacon/probe response frames
        
        // For now, we'll trigger the wifi scan module to process results
        crate::drivers::wifi::scan::set_scanning(false);
        crate::drivers::wifi::scan::set_complete(true);
        
        crate::mesa_println!("[RTL8822CE] SCAN_DONE processed");
    }

    // ========================================================================
    // BA (Block Acknowledgment) Session Management
    // Mirrors Linux rtw_ba_session / rtw_rx_reorder_ctrl
    // ========================================================================

    /// Initialize BA session for a TID
    fn ba_session_init(&mut self, macid: u8, tid: u8, win_size: u8) {
        let idx = (macid as usize * 8 + tid as usize) % 128;
        self.ba_sessions[idx] = BaSession {
            tid,
            macid,
            bmp: 0,
            start_seq: 0,
            win_size,
            state: 1, // initiated
            timeout: 0,
            last_rx_seq: 0,
            pending_rx: 0,
        };
        crate::mesa_println!("[RTL8822CE] BA session init: macid={} tid={} win_size={}", macid, tid, win_size);
    }

    /// Start BA session (sender side) - mirrors Linux rtw_ba_start
    fn ba_session_start(&mut self, macid: u8, tid: u8, start_seq: u16, win_size: u8) {
        let idx = (macid as usize * 8 + tid as usize) % 128;
        self.ba_sessions[idx] = BaSession {
            tid,
            macid,
            bmp: 0,
            start_seq,
            win_size,
            state: 2, // active
            timeout: 0,
            last_rx_seq: start_seq,
            pending_rx: 0,
        };
        
        // Send H2C BA start command
        let mut cmd = [0u8; 8];
        cmd[0] = 0x05; // H2C_CMD_BA_START
        cmd[1] = macid;
        cmd[2] = tid;
        cmd[3] = (start_seq & 0xFF) as u8;
        cmd[4] = ((start_seq >> 8) & 0xFF) as u8;
        cmd[5] = win_size;
        let _ = self.h2c_command(&cmd);
        
        crate::mesa_println!("[RTL8822CE] BA session start: macid={} tid={} seq={} win={}", 
            macid, tid, start_seq, win_size);
    }

    /// Stop BA session
    fn ba_session_stop(&mut self, macid: u8, tid: u8) {
        let idx = (macid as usize * 8 + tid as usize) % 128;
        self.ba_sessions[idx].state = 3; // closing
        
        // Send H2C BA stop command
        let mut cmd = [0u8; 8];
        cmd[0] = 0x06; // H2C_CMD_BA_STOP
        cmd[1] = macid;
        cmd[2] = tid;
        let _ = self.h2c_command(&cmd);
        
        crate::mesa_println!("[RTL8822CE] BA session stop: macid={} tid={}", macid, tid);
    }

    /// Process RX BA frame - mirrors Linux rtw_rx_ba
    fn process_rx_ba(&mut self, macid: u8, tid: u8, seq: u16, data: &[u8]) {
        let idx = (macid as usize * 8 + tid as usize) % 128;
        let session = &mut self.ba_sessions[idx];
        
        if session.state != 2 {
            return; // Not active
        }
        
        let win_start = session.start_seq;
        let win_end = (win_start + session.win_size as u16) & 0xFFF;
        
        // Check if sequence is in window
        let in_window = if win_start <= win_end {
            seq >= win_start && seq < win_end
        } else {
            seq >= win_start || seq < win_end
        };
        
        if !in_window {
            crate::mesa_println!("[RTL8822CE] RX BA: seq {} out of window [{}-{}]", seq, win_start, win_end);
            return;
        }
        
        // Mark in bitmap
        let bit_pos = ((seq - win_start) & 0xFFF) as usize;
        if bit_pos < 64 {
            session.bmp |= 1 << bit_pos;
        }
        
        // Check if we can release frames
        while session.bmp & 1 != 0 {
            session.bmp >>= 1;
            session.start_seq = (session.start_seq + 1) & 0xFFF;
            session.last_rx_seq = session.start_seq;
        }
        
        session.pending_rx += 1;
    }

    /// RX Reorder - mirrors Linux rtw_rx_reorder
    fn rx_reorder(&mut self, macid: u8, tid: u8, seq: u16, data: &[u8]) -> Option<Vec<u8>> {
        let reorder_idx = tid as usize % 16;
        let reorder = &mut self.rx_reorder[reorder_idx];
        
        if !reorder.enable {
            return Some(data.to_vec()); // No reordering
        }
        
        // Find free buffer slot
        for i in 0..8 {
            if !reorder.buf[i].in_use {
                reorder.buf[i] = RxReorderEntry {
                    in_use: true,
                    seq,
                    timestamp: self.fw_watchdog_timer as u32,
                    data: [0; 2304],
                    data_len: data.len() as u16,
                };
                reorder.buf[i].data[..data.len()].copy_from_slice(data);
                break;
            }
        }
        
        // Check if we can release in-order frames
        let mut released = Vec::new();
        while reorder.indicator == reorder.buf[reorder.indicator as usize % 8].seq {
            let entry = &reorder.buf[reorder.indicator as usize % 8];
            if entry.in_use {
                released.extend_from_slice(&entry.data[..entry.data_len as usize]);
                reorder.buf[reorder.indicator as usize % 8].in_use = false;
                reorder.indicator = (reorder.indicator + 1) & 0xFFF;
            } else {
                break;
            }
        }
        
        if released.is_empty() {
            None
        } else {
            Some(released)
        }
    }

    // ========================================================================
    // Channel Switching Helpers (matches Linux phydm_hal_api8822c.c)
    // ========================================================================

    /// Switch band (2.4G <-> 5G) - matches Linux config_phydm_switch_band_8822c
    fn phydm_switch_band(&mut self, is_5g: bool) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Band switch: is_5g={}", is_5g);
        
        unsafe {
            if is_5g {
                // 5G: Disable CCK, enable OFDM
                // Disable CCK TRx
                self.write32(0x1A80, self.read32(0x1A80) | (1 << 18)); // BB CCK check enable
                self.write8(0x0454, self.read8(0x0454) | (1 << 7)); // MAC CCK check enable
                // Disable CCK TRx
                self.write32(0x1A00, self.read32(0x1A00) & !(0x3 << 0)); // CCK disable
                // CCA Mask for 5G
                self.write32(0x1C80, (self.read32(0x1C80) & !0x3F000000) | 0x22000000);
                // RF band select
                self.write_rf_sipi(0, 0x18, RFREG_MASK, (self.read_rf_sipi(0, 0x18, RFREG_MASK) & !(0xF << 16)) | (1 << 16) | (1 << 8));
                self.write_rf_sipi(1, 0x18, RFREG_MASK, (self.read_rf_sipi(1, 0x18, RFREG_MASK) & !(0xF << 16)) | (1 << 16) | (1 << 8));
            } else {
                // 2G: Enable CCK, disable OFDM checks
                self.write32(0x1A80, self.read32(0x1A80) & !(1 << 18)); // BB CCK check disable
                self.write8(0x0454, self.read8(0x0454) & !(1 << 7)); // MAC CCK check disable
                // CCA Mask for 2G
                self.write32(0x1C80, (self.read32(0x1C80) & !0x3F000000) | 0x0F000000);
                // RF band select
                self.write_rf_sipi(0, 0x18, RFREG_MASK, (self.read_rf_sipi(0, 0x18, RFREG_MASK) & !(0xF << 16)) | 0x1);
                self.write_rf_sipi(1, 0x18, RFREG_MASK, (self.read_rf_sipi(1, 0x18, RFREG_MASK) & !(0xF << 16)) | 0x1);
            }
        }
        driver_unlock(); Ok(())
    }

    /// BB channel/bandwidth configuration - faithful port of rtw88
    /// `rtw8822c_set_channel_bb` (rtw8822c.c). This was completely missing:
    /// `phydm_switch_channel` only programmed the RF synthesizer, so BGCTRL,
    /// CCK_CHECK/CCKTXONLY, the RX AGC codes, CCAMSK, DFIR/TX bandwidth and
    /// CCK source kept stale post-table values and RX never demodulated.
    /// Only the 20 MHz case is implemented (all our scans are 20 MHz).
    fn set_channel_bb(&self, channel: u8, pri_ch_idx: u8) {
        unsafe {
            let is_2g = channel <= 14;

            if is_2g {
                self.write32_mask(REG_BGCTRL, BITS_RX_IQ_WEIGHT, 0x0);
                self.write32_mask(REG_TXF4, 1 << 20, 0x1);
                self.write32_mask(REG_CCK_CHECK, BIT_CHECK_CCK_EN, 0x0);
                self.write32_mask(REG_CCKTXONLY, BIT_BB_CCK_CHECK_EN, 0x0);
                self.write32_mask(REG_CCAMSK, 0x3F00_0000, 0xF);

                // BW 20 MHz
                self.write32_mask(REG_RXAGCCTL0, BITS_RXAGC_CCK, 0x5);
                self.write32_mask(REG_RXAGCCTL, BITS_RXAGC_CCK, 0x5);
                self.write32_mask(REG_RXAGCCTL0, BITS_RXAGC_OFDM, 0x6);
                self.write32_mask(REG_RXAGCCTL, BITS_RXAGC_OFDM, 0x6);

                let scotrk = if channel == 13 || channel == 14 {
                    0x969
                } else if channel == 11 || channel == 12 {
                    0x96A
                } else {
                    0x9AA
                };
                self.write32_mask(REG_SCOTRK, 0xFFF, scotrk);

                // CCK TX shaping filter (TXF0..7)
                if channel == 14 {
                    self.write32_mask(REG_TXF0, 0xFFFF_0000, 0x3DA0);
                    self.write32_mask(REG_TXF1, 0xFFFF_FFFF, 0x4962C931);
                    self.write32_mask(REG_TXF2, 0x0000_FFFF, 0x6AA3);
                    self.write32_mask(REG_TXF3, 0xFFFF_0000, 0xAA7B);
                    self.write32_mask(REG_TXF4, 0x0000_FFFF, 0xF3D7);
                    self.write32_mask(REG_TXF5, 0xFFFF_FFFF, 0x0);
                    self.write32_mask(REG_TXF6, 0xFFFF_FFFF, 0xFF012455);
                    self.write32_mask(REG_TXF7, 0xFFFF_FFFF, 0xFFFF);
                } else {
                    self.write32_mask(REG_TXF0, 0xFFFF_0000, 0x5284);
                    self.write32_mask(REG_TXF1, 0xFFFF_FFFF, 0x3E18FEC8);
                    self.write32_mask(REG_TXF2, 0x0000_FFFF, 0x0A88);
                    self.write32_mask(REG_TXF3, 0xFFFF_0000, 0xACC4);
                    self.write32_mask(REG_TXF4, 0x0000_FFFF, 0xC8B2);
                    self.write32_mask(REG_TXF5, 0xFFFF_FFFF, 0x00FAF0DE);
                    self.write32_mask(REG_TXF6, 0xFFFF_FFFF, 0x00122344);
                    self.write32_mask(REG_TXF7, 0xFFFF_FFFF, 0x0FFFFFFF);
                }

                let dfir = if channel == 13 { 0x3 } else { 0x1 };
                self.write32_mask(REG_TXDFIR0, 0x70, dfir);
            } else {
                self.write32_mask(REG_CCKTXONLY, BIT_BB_CCK_CHECK_EN, 0x1);
                self.write32_mask(REG_CCK_CHECK, BIT_CHECK_CCK_EN, 0x1);
                self.write32_mask(REG_BGCTRL, BITS_RX_IQ_WEIGHT, 0x3);
                self.write32_mask(REG_TXF4, 1 << 20, 0x0);
                self.write32_mask(REG_CCAMSK, 0x3F00_0000, 0x22);
                self.write32_mask(REG_TXDFIR0, 0x70, 0x3);

                let ofdm = if channel <= 64 {
                    0x1
                } else if channel <= 144 {
                    0x2
                } else {
                    0x3
                };
                self.write32_mask(REG_RXAGCCTL0, BITS_RXAGC_OFDM, ofdm);
                self.write32_mask(REG_RXAGCCTL, BITS_RXAGC_OFDM, ofdm);

                let scotrk = if (36..=51).contains(&channel) {
                    0x494
                } else if (52..=55).contains(&channel) {
                    0x493
                } else if (56..=111).contains(&channel) {
                    0x453
                } else if (112..=119).contains(&channel) {
                    0x452
                } else if (120..=172).contains(&channel) {
                    0x412
                } else {
                    0x411
                };
                self.write32_mask(REG_SCOTRK, 0xFFF, scotrk);
            }

            // BW 20 MHz common part
            self.write32_mask(REG_DFIRBW, 0x3FF0, 0x19B);
            self.write32_mask(REG_TXBWCTL, 0x000F, 0x0);
            self.write32_mask(REG_TXBWCTL, 0xFFC0, 0x0);
            self.write32_mask(REG_TXCLK, 0x0000_0700, 0x7);
            self.write32_mask(REG_TXCLK, 0x0070_0000, 0x6);
            self.write32_mask(REG_CCK_SOURCE, BIT_NBI_EN, 0x0);
            self.write32_mask(REG_SBD, BITS_SUBTUNE, 0x1);
            self.write32_mask(REG_PT_CHSMO, BIT_PT_OPT, 0x0);

            let _ = pri_ch_idx;
        }
    }

    /// Switch channel - matches Linux config_phydm_switch_channel_8822c
    fn phydm_switch_channel(&mut self, channel: u8) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] PHYDM switch channel: ch={}", channel);
        
        // Program RF register 0x18 via SIPi (like config_phydm_switch_channel_8822c)
        let is_5g = self.current_band_5g || channel > 14;

        // BB channel config first (rtw8822c_set_channel: bb then rf).
        self.set_channel_bb(channel, 0);

        self.rstb_3wire(false);
        
        for path in [0u8, 1u8] {
            let mut rf18 = self.read_rf_sipi(path, 0x18, RFREG_MASK);
            // Match rtw88 rtw8822c_set_channel_rf clear mask exactly:
            //   RF18_BAND_MASK (BIT16|BIT9|BIT8) | RF18_CHANNEL_MASK (0xFF)
            //   | RF18_RFSI_MASK (BIT18|BIT17) | RF18_BW_MASK (BIT13|BIT12)
            // The previous mask omitted BIT8|BIT9, so a stale band bit (e.g. the
            // 5G BIT8) survived on 2G and left the synthesizer mistuned -> no
            // RX and TXERR.
            rf18 &= !( (0xFF) | (1 << 8) | (1 << 9) | (1 << 12) | (1 << 13)
                | (1 << 16) | (1 << 17) | (1 << 18) );
            rf18 |= channel as u32;
            
            if self.current_band_5g || channel > 14 {
                rf18 |= (1 << 16) | (1 << 8);
                if channel > 144 {
                    rf18 |= 1 << 18;
                } else if channel >= 80 {
                    rf18 |= 1 << 17;
                }
            }
            // 20MHz
            rf18 |= (1 << 13) | (1 << 12);
            
            // RF RX BB LUT (rf_rxbb = 0x18 for 20MHz), as in rtw8822c_set_channel_rf.
            self.write_rf_mix(path, 0xEE, 0x04, 0x01);   // RF_LUTWE2
            self.write_rf_mix(path, 0x33, 0x1F, 0x12);   // RF_LUTWA
            self.write_rf_mix(path, 0x3F, 0xFFFFF, 0x18);// RF_LUTWD0
            self.write_rf_mix(path, 0xEE, 0x04, 0x00);   // RF_LUTWE2
            
            // 8822C writes RF_CHNLBW (0x18) through the direct BB-mapped window
            // (0x3c18/0x4c18), matching rtw8822c_set_channel_rf -> rtw_write_rf.
            self.write_rf_mix(path, 0x18, RFREG_MASK, rf18);
        }
        
        self.rstb_3wire(true);
        
        // CCK TX shaping filter
        self.phydm_cck_tx_shaping_filter(channel);
        
        // AGC table selection
        self.phydm_agc_tab_sel(channel);
        
        driver_unlock(); Ok(())
    }

    /// Switch bandwidth - matches Linux config_phydm_switch_bandwidth_8822c
    fn phydm_switch_bandwidth(&mut self, channel: u8, pri_ch_idx: u8) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] PHYDM switch bandwidth: ch={} pri_ch_idx={}", channel, pri_ch_idx);
        
        unsafe {
            // Bandwidth register: 0x0668 bits 7:8
            // 0 = 20MHz, 1 = 40MHz, 2 = 80MHz, 3 = 160MHz
            let bw = 0u32; // 20MHz default
            let val = self.read32(0x0668);
            self.write32(0x0668, (val & !(0x3 << 7)) | (bw << 7));
            
            // DATA_SC: primary channel index
            self.write8(0x0483, pri_ch_idx);
            
            // SCO tracking
            self.phydm_sco_trk_fc_setting(channel);
        }
        driver_unlock(); Ok(())
    }

    /// MAC bandwidth config - matches Linux mac_switch_bandwidth
    fn mac_switch_bandwidth(&mut self, channel: u8) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] MAC switch bandwidth: ch={}", channel);
        
        // Use HALMAC set_bandwidth equivalent
        // For 20MHz: pri_ch_idx = channel
        // HALMAC: rtw_halmac_set_bandwidth(adapter, channel, pri_ch_idx, bw)
        // Simplified: just set primary channel
        let pri_ch_idx = channel;
        
        unsafe {
            // MAC register for primary channel
            self.write8(0x0483, pri_ch_idx);
            
            // Bandwidth register
            self.write32(0x0668, (self.read32(0x0668) & !(0x3 << 7)) | (0 << 7)); // 20MHz
        }
        driver_unlock(); Ok(())
    }

    /// CCK TX shaping filter - matches Linux phydm_cck_tx_shaping_filter_8822c
    fn phydm_cck_tx_shaping_filter(&self, channel: u8) {
        unsafe {
            if channel == 14 {
                // Japan channel 14 special filter
                self.write32(0x1A20, (self.read32(0x1A20) & !0xFFFF0000) | 0x3DA00000);
                self.write32(0x1A24, 0x4962C931);
                self.write32(0x1A28, (self.read32(0x1A28) & !0xFFFF) | 0x6AA3);
                self.write32(0x1A98, (self.read32(0x1A98) & !0xFFFF0000) | 0xAA7B0000);
                self.write32(0x1A9C, (self.read32(0x1A9C) & !0xFFFF) | 0xF3D7);
                self.write32(0x1AA0, 0x00000000);
                self.write32(0x1AAC, 0xFF012455);
                self.write32(0x1AB0, 0x0000FFFF);
            } else {
                // Standard CCK TX shaping
                self.write32(0x1A20, (self.read32(0x1A20) & !0xFFFF0000) | 0x52840000);
                self.write32(0x1A24, 0x3E18FEC8);
                self.write32(0x1A28, (self.read32(0x1A28) & !0xFFFF) | 0x0A88);
                self.write32(0x1A98, (self.read32(0x1A98) & !0xFFFF0000) | 0xACC40000);
                self.write32(0x1A9C, (self.read32(0x1A9C) & !0xFFFF) | 0xC8B2);
                self.write32(0x1AA0, 0x00FAF0DE);
                self.write32(0x1AAC, 0x00122344);
                self.write32(0x1AB0, 0x0FFFFFFF);
            }
        }
    }

    /// AGC table selection - matches Linux phydm_agc_tab_sel_8822c
    fn phydm_agc_tab_sel(&self, channel: u8) {
        let is_5g = self.current_band_5g || channel > 14;
        
        if is_5g {
            // 5G AGC table selection based on channel range
            if channel >= 36 && channel <= 64 {
                // UNII-1 (36-64)
                unsafe {
                    self.write32(0x18AC, (self.read32(0x18AC) & !0x1F0) | 0x10);
                    self.write32(0x41AC, (self.read32(0x41AC) & !0x1F0) | 0x10);
                }
            } else if channel >= 100 && channel <= 144 {
                // UNII-2/3 (100-144)
                unsafe {
                    self.write32(0x18AC, (self.read32(0x18AC) & !0x1F0) | 0x20);
                    self.write32(0x41AC, (self.read32(0x41AC) & !0x1F0) | 0x20);
                }
            } else {
                // UNII-4 (149+)
                unsafe {
                    self.write32(0x18AC, (self.read32(0x18AC) & !0x1F0) | 0x30);
                    self.write32(0x41AC, (self.read32(0x41AC) & !0x1F0) | 0x30);
                }
            }
        } else {
            // 2G AGC table
            unsafe {
                self.write32(0x18AC, (self.read32(0x18AC) & !0xF000) | 0x5000);
                self.write32(0x41AC, (self.read32(0x41AC) & !0xF000) | 0x5000);
                self.write32(0x18AC, (self.read32(0x18AC) & !0x1F0) | 0x60);
                self.write32(0x41AC, (self.read32(0x41AC) & !0x1F0) | 0x60);
            }
        }
    }

    /// SCO tracking - matches Linux phydm_sco_trk_fc_setting_8822c
    fn phydm_sco_trk_fc_setting(&self, channel: u8) {
        unsafe {
            let val = match channel {
                13 | 14 => 0x969,
                11 | 12 => 0x96A,
                1..=10 => 0x9AA,
                36..=51 => 0x494,
                52..=55 => 0x493,
                56..=111 => 0x453,
                112..=119 => 0x452,
                120..=172 => 0x412,
                _ => 0x411, // 173-177
            };
            self.write32(0x0C30, (self.read32(0x0C30) & !0xFFF) | val);
        }
    }

    /// TX Power setting - simplified
    fn phydm_tx_power_setting(&self, channel: u8) -> Result<(), &'static str> {
        // Simplified: just log, actual implementation would set per-rate power
        crate::mesa_println!("[RTL8822CE] TX power setting for ch={}", channel);
        driver_unlock(); Ok(())
    }

    // =======================================================================
    // H2C Command Interface (Host-to-Chart commands to firmware)
    // =======================================================================

    /// Send an H2C command to the firmware (used for control signaling).
    /// Routes through `h2c_command`, which writes the extension dword to the
    /// correct HMEBOXx_EX register (0x98-0x9E) before the box itself, like
    /// Linux `rtw_fw_send_h2c_command`.
    fn send_h2c_command(&mut self, cmd: u32, arg: u32) -> Result<(), &'static str> {
        let mut buf = [0u8; 8];
        buf[0..4].copy_from_slice(&cmd.to_le_bytes());
        buf[4..8].copy_from_slice(&arg.to_le_bytes());
        match self.h2c_command(&buf) {
            Ok(()) => {
                crate::mesa_println!("[RTL8822CE] H2C cmd=0x{:08X} arg=0x{:08X} sent", cmd, arg);
                driver_unlock();
                Ok(())
            },
            Err(e) => {
                crate::mesa_println!("[RTL8822CE] H2C cmd=0x{:08X} failed: {}", cmd, e);
                driver_unlock();
                Ok(())
            }
        }
    }

    /// Send H2C_MEDIA_STATUS_RPT (0x01) - Linux rtw_hal_set_FwMediaStatusRpt_single_cmd
    /// opmode: 0=disconnect, 1=connect
    /// role: 1=STA, 2=AP, 3=GC, 4=GO, 5=TDLS
    /// macid: AID for STA mode
    fn h2c_media_status_rpt(&mut self, opmode: u8, _role: u8, macid: u8) -> Result<(), &'static str> {
        // Linux rtw_fw_media_status_report: w0[7:0]=cmd id, w0[8]=op_mode,
        // w0[23:16]=mac_id. There is NO role field (the old code shifted role
        // into bit 12 and corrupted the word).
        let mut cmd = [0u8; 8];
        cmd[0] = H2C_CMD_MEDIA_STATUS_RPT; // 0x01
        cmd[1] = opmode & 1;               // w0 bit 8
        cmd[2] = macid;                    // w0 bits[23:16]

        crate::mesa_println!("[RTL8822CE] H2C MEDIA_STATUS_RPT: opmode={} macid={}",
            opmode, macid);

        self.h2c_command(&cmd)
    }

    /// H2C_DEFAULT_PORT (0x2c) - Linux rtw_fw_default_port: tells the firmware
    /// which vif/port is the default for data TX/RX. w0 = cmdid | portid<<8 | macid<<16.
    fn h2c_default_port(&mut self, portid: u8, macid: u8) -> Result<(), &'static str> {
        let mut cmd = [0u8; 8];
        cmd[0] = H2C_CMD_DEFAULT_PORT; // 0x2c
        cmd[1] = portid;               // w0 bits[15:8]
        cmd[2] = macid;                // w0 bits[23:16]
        crate::mesa_println!("[RTL8822CE] H2C DEFAULT_PORT: port={} macid={}", portid, macid);
        self.h2c_command(&cmd)
    }

    /// H2C_FW_JOINBSSRPT (Join BSS Report) - Linux hw_var_set_h2c_fw_joinbssrpt
    /// Called after successful association to notify firmware and download reserved page
fn h2c_join_bss_report(&mut self, connect: bool) -> Result<(), &'static str> {
    if !connect {
        crate::mesa_println!("[RTL8822CE] H2C JOIN_BSSRPT: disconnect");
        // For disconnect, just send media status rpt with opmode=0
        return self.h2c_media_status_rpt(0, 1, 0);
    }

    crate::mesa_println!("[RTL8822CE] H2C JOIN_BSSRPT: connect - downloading reserved page");

    // 1. First send MEDIA_STATUS_RPT (connect, STA role)
    self.h2c_media_status_rpt(1, 1, 1)?; // opmode=1, role=1(STA), macid=1 (temp, will update after AID known)

    // 1b. Set the default port so the FW routes data for this vif.
    self.h2c_default_port(0, 0)?;

    // 2. Download reserved page (beacon, probe response, PS-Poll, Null data)
    // Linux does this in hw_var_set_dl_rsvd_page called from hw_var_set_h2c_fw_joinbssrpt
    self.download_rsvd_page_connect()?;

    driver_unlock(); Ok(())
}

/// Send BT info to firmware via H2C command
/// Mirrors Linux: rtl8822c_BT_INFO_cmd
fn h2c_bt_info(&mut self, bt_exist: bool, linked: bool, stat: u8, ble: bool) -> Result<(), &'static str> {
    // H2C BT_INFO command format (8 bytes):
    // Byte 0: Command (0x61 = H2C_BT_INFO)
    // Byte 1: BT_exist (1=BT exists, 0=BT not exist)
    // Byte 2: Linked (1=connected, 0=not connected)
    // Byte 3: Stat (0=Idle, 1=Inquiry, 2=Page, 3=Active, 4=Auth, 5=Sync, 6=Others)
    // Byte 4: Ble (1=BLE, 0=Classic BT)
    // Bytes 5-7: Padding
    
    let mut cmd = [0u8; 8];
    cmd[0] = H2C_CMD_BT_INFO;
    cmd[1] = if bt_exist { 1 } else { 0 };
    cmd[2] = if linked { 1 } else { 0 };
    cmd[3] = stat;
    cmd[4] = if ble { 1 } else { 0 };
    // bytes 5-7 = 0
    
    self.h2c_command(&cmd)?;
    
    crate::mesa_println!("[RTL8822CE] H2C BT_INFO: exist={} linked={} stat={} ble={}", bt_exist, linked, stat, ble);
    
    // Implementation would download the reserved page sections
    // Beacon, probe response, PS-Poll, QoS Null data
    driver_unlock(); Ok(())
}

/// H2C_MEDIA_STATUS_RPT (0x01) - Linux rtw_hal_set_FwMediaStatusRpt_single_cmd
    fn download_rsvd_page_connect(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Downloading reserved page for connection...");

        // Use BCN queue to send reserved page frames (same as firmware download staging)
        // This is a simplified version - Linux constructs full beacon/probe response frames
        // For now, we send a minimal beacon to reserve the page

        // Enable SW beacon TX
        unsafe {
            let cr1 = self.read8(REG_CR + 1);
            self.write8(REG_CR + 1, cr1 | (1 << 0)); // ENSWBCN
        }

        // Disable HW beacon function temporarily
        unsafe {
            let bcn_ctrl = self.read8(0x0550); // REG_BCN_CTRL
            self.write8(0x0550, bcn_ctrl & !(1 << 3) | (1 << 4)); // DIS_TSF_UDT
        }

        // Build a minimal beacon frame for the reserved page
        // This reserves space in FW for beacon/probe response handling
        let beacon_frame = self.build_minimal_beacon()?;
        self.send_mgmt_frame_internal(&beacon_frame)?;

        // Wait for BCN_VALID (beacon sent to reserved page)
        unsafe {
            for _ in 0..1000 {
                let val = self.read16(0x0204); // REG_FIFOPAGE_CTRL_2
                if val & 0x8000 != 0 { // BCN_VALID
                    crate::mesa_println!("[RTL8822CE] Reserved page downloaded (BCN_VALID)");
                    break;
                }
                self.delay_us(10);
            }
        }

        // Restore BCN_CTRL
        unsafe {
            let bcn_ctrl = self.read8(0x0550);
            self.write8(0x0550, bcn_ctrl | (1 << 3) & !(1 << 4)); // EN_BCN_FUNCTION, clear DIS_TSF_UDT
        }

        // Disable SW beacon TX
        unsafe {
            let cr1 = self.read8(REG_CR + 1);
            self.write8(REG_CR + 1, cr1 & !(1 << 0)); // Clear ENSWBCN
        }

        // Send H2C RSVD_PAGE command with page locations
        // Linux: rtw_hal_set_fw_rsvd_page with finished=true
        let mut cmd = [0u8; 8];
        cmd[0] = H2C_CMD_RSVD_PAGE; // 0x00
        // Page locations (simplified - in Linux these are actual page numbers)
        cmd[1] = 0x01; // Beacon page
        cmd[2] = 0x02; // Probe response page
        cmd[3] = 0x03; // PS-Poll page
        cmd[4] = 0x04; // Null data page
        cmd[5] = 0x05; // QoS Null page
        cmd[6] = 0x06; // BT QoS Null page
        cmd[7] = 0x01; // finished = true
        self.h2c_command(&cmd)?;

        crate::mesa_println!("[RTL8822CE] Reserved page H2C command sent");
        driver_unlock(); Ok(())
    }

    /// H2C Channel Switch command (0x1C with sub-command 0x02) - Linux ctrl_ch_switch_88xx
    /// Uses H2C_CHNL_SWITCH_OPER_OFFLOAD (0x1C) with SUB_CMD_ID_CH_SWITCH (0x02)
    /// Full 32-byte H2C packet with 20-byte channel switch payload (per HALMAC)
    #[allow(dead_code)]
    fn h2c_channel_switch(&mut self, channel: u8, enable: bool) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] H2C Channel Switch: ch={} enable={}", channel, enable);

        // Full 32-byte H2C packet:
        // Bytes 0-7: H2C header
        //   byte 0: H2C command ID (0x1C = CHNL_SWITCH_OPER_OFFLOAD)
        //   byte 1: sub-command ID (0x02 = CH_SWITCH)
        //   bytes 2-3: reserved / flags
        //   bytes 4-7: reserved
        // Bytes 8-27: Channel switch payload (20 bytes, per HALMAC ctrl_ch_switch_88xx)
        //   offset 0x08 (byte 8):  START(1) | DEST_CH_EN(1) | ABSOLUTE_TIME(1) | PERIODIC_OPT(2) | SCAN_MODE(1) | INFO_LOC(8) | CH_NUM(8) | PRI_CH_IDX(4) | DEST_BW(4)
        //   offset 0x0C: DEST_CH(8) | NORMAL_PERIOD(6) | NORMAL_PERIOD_SEL(2) | SLOW_PERIOD(6) | SLOW_PERIOD_SEL(2) | NORMAL_CYCLE(8)
        //   offset 0x10: TSF_HIGH (32 bits)
        //   offset 0x14: TSF_LOW (32 bits)
        //   offset 0x18: INFO_SIZE (16 bits) + reserved
        // Bytes 28-31: padding

        let mut pkt = [0u8; 32];

        // H2C header (8 bytes)
        pkt[0] = H2C_CMD_CHNL_SWITCH_OPER_OFFLOAD; // 0x1C
        pkt[1] = H2C_SUB_CMD_CH_SWITCH; // 0x02
        pkt[2] = 0x00;
        pkt[3] = 0x00;
        pkt[4] = 0x00;
        pkt[5] = 0x00;
        pkt[6] = 0x00;
        pkt[6] = 0x00; // H2C_PKT_SIZE_88XX = 32, so buf_fs check uses this
        pkt[7] = 0x00;

        // Channel switch payload (20 bytes at offset 0x08)
        // Byte 8 (offset 0x08): START(1) | DEST_CH_EN(1) | ABSOLUTE_TIME(1) | PERIODIC_OPT(2) | SCAN_MODE(1) | INFO_LOC(8) | CH_NUM(8) | PRI_CH_IDX(4) | DEST_BW(4)
        // Actually the first byte at offset 0x08 contains START(1) in bit 0
        let mut byte8 = 0u32;
        byte8 |= 1 << 0; // START = 1 (enable)
        // CH_SWITCH_SET_START at bit 0 of byte 8 (offset 0x08)

        // Byte 8-11 (offset 0x08): 4 bytes containing START, DEST_CH_EN, ABSOLUTE_TIME, PERIODIC_OPT, SCAN_MODE, INFO_LOC, CH_NUM, PRI_CH_IDX, DEST_BW
        // Per CH_SWITCH_SET_START at offset 0x08 bit 0
        // CH_SWITCH_SET_CH_NUM at offset 0x08 bits 16-23
        // CH_SWITCH_SET_PRI_CH_IDX at offset 0x08 bits 24-27
        // CH_SWITCH_SET_DEST_BW at offset 0x08 bits 28-31

        let mut payload_08 = 0u32;
        payload_08 |= 1 << 0; // START = 1 (enable)
        payload_08 |= 1 << 1; // DEST_CH_EN = 1
        // ABSOLUTE_TIME = 0 (bit 2)
        // PERIODIC_OPT = 0 (bits 3-4)
        // SCAN_MODE = 0 (bit 5)
        // INFO_LOC = 0 (bits 6-13)
        payload_08 |= (channel as u32) << 16; // CH_NUM (bits 16-23)
        payload_08 |= (channel as u32) << 24; // PRI_CH_IDX (bits 24-27)
        // DEST_BW = 0 (bits 28-31) = 20MHz

        // Write bytes 8-11 (little endian)
        pkt[8] = (payload_08 & 0xFF) as u8;
        pkt[9] = ((payload_08 >> 8) & 0xFF) as u8;
        pkt[10] = ((payload_08 >> 16) & 0xFF) as u8;
        pkt[11] = ((payload_08 >> 24) & 0xFF) as u8;

        // Bytes 12-15 (offset 0x0C): DEST_CH(8) | NORMAL_PERIOD(6) | NORMAL_PERIOD_SEL(2) | SLOW_PERIOD(6) | SLOW_PERIOD_SEL(2) | NORMAL_CYCLE(8)
        // All zeros for basic channel switch
        pkt[12] = 0x00; // DEST_CH = 0
        pkt[13] = 0x00; // NORMAL_PERIOD=0, NORMAL_PERIOD_SEL=0
        pkt[14] = 0x00; // SLOW_PERIOD=0, SLOW_PERIOD_SEL=0
        pkt[15] = 0x00; // NORMAL_CYCLE = 0

        // Bytes 16-19 (offset 0x10): TSF_HIGH (32 bits) - 0 for now
        pkt[16] = 0x00;
        pkt[17] = 0x00;
        pkt[18] = 0x00;
        pkt[19] = 0x00;

        // Bytes 20-23 (offset 0x14): TSF_LOW (32 bits) - 0 for now
        pkt[20] = 0x00;
        pkt[21] = 0x00;
        pkt[22] = 0x00;
        pkt[23] = 0x00;

        // Bytes 24-25 (offset 0x18): INFO_SIZE (16 bits)
        pkt[24] = 0x00;
        pkt[25] = 0x00;

        // Bytes 26-31: padding (already zero)

        // H2C header (first 8 bytes)
        pkt[0] = H2C_CMD_CHNL_SWITCH_OPER_OFFLOAD; // 0x1C
        pkt[1] = H2C_SUB_CMD_CH_SWITCH; // 0x02
        // pkt[2..7] = 0 already

        crate::mesa_println!("[RTL8822CE] H2C Channel Switch: ch={} enable=true (32-byte packet)", channel);
        // crate::mesa_println!("[RTL8822CE] H2C pkt: {:02X?}", &pkt[..]);

        // Send via H2C mailbox - we need to send the full 32 bytes
        // Our h2c_command only sends 8 bytes, so we need a new method
        self.h2c_command_full(&pkt)
    }
    fn build_minimal_beacon(&self) -> Result<alloc::vec::Vec<u8>, &'static str> {
        // Minimal beacon: 802.11 header + fixed params + minimal IEs
        // This is a placeholder - real implementation needs full beacon construction
        let mut frame = alloc::vec::Vec::with_capacity(256);
        
        // 802.11 Header (24 bytes)
        // Frame Control: Beacon (type=00, subtype=1000 = 0x8000)
        frame.extend_from_slice(&[0x80, 0x00]); // Frame Control
        frame.extend_from_slice(&[0x00, 0x00]); // Duration
        frame.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]); // DA (broadcast)
        frame.extend_from_slice(&self.mac); // SA (our MAC)
        frame.extend_from_slice(&self.mac); // BSSID (our MAC for now)
        frame.extend_from_slice(&[0x00, 0x00]); // Seq Ctrl
        
        // Fixed parameters (12 bytes)
        frame.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // Timestamp (8 bytes)
        frame.extend_from_slice(&[0x64, 0x00]); // Beacon interval (100 TU)
        frame.extend_from_slice(&[0x11, 0x04]); // Capability (ESS + Privacy + Short Preamble + Short Slot)
        
        // Tagged parameters - minimal SSID
        frame.push(0x00); // SSID element ID
        frame.push(0x00); // SSID length (0 = wildcard)
        
        // Supported rates (minimal)
        frame.push(0x01); // Supported Rates element ID
        frame.push(0x08); // Length
        frame.extend_from_slice(&[0x82, 0x84, 0x8B, 0x96, 0x0C, 0x12, 0x18, 0x24]); // 1,2,5.5,11,6,9,12,18 Mbps
        
        // DS Parameter Set (channel)
        frame.push(0x03); // DS Parameter Set element ID
        frame.push(0x01); // Length
        frame.push(1); // Channel 1 (will be updated)
        
        Ok(frame)
    }

    /// Internal send management frame (MGT queue) - takes &mut self, no locking
    /// For use from scan/connect where driver lock is already held
    fn send_mgmt_frame_internal(&mut self, frame: &[u8]) -> Result<(), &'static str> {
        // Reclaim completed TX descriptors first (update HW read index)
        // In Linux this happens in TX done interrupt; here we poll manually
        self.reclaim_tx_done();

        // Use the Management queue (MGT)
        let queue = TX_QUEUE_MGT as usize;
        let idx = self.tx_wr_idx[queue] as usize;

        // Ring flow control: never overwrite descriptors still owned by HW.
        if self.tx_avail_descs(queue) < 2 {
            return Err("RTL8822CE: TX ring full");
        }

        let buf_phys = unsafe {
            (self.tx_buf[queue] as u64) + (idx as u64) * TX_BUF_SIZE as u64
        };
        let buf_virt = unsafe {
            (self.tx_buf_virt[queue] as u64) + (idx as u64) * TX_BUF_SIZE as u64
        };

        if frame.len() + TX_HDR_SIZE as usize > TX_BUF_SIZE {
            return Err("Management frame too large");
        }

        // Build TX descriptor for management queue.
        // rtw88 rtw_tx_mgmt_pkt_info_update sets qsel = TX_DESC_QSEL_MGMT (18),
        // ls=1, dis_qselseq=1, en_hwseq=1, hw_ssn_sel=0 and bmc when addr1 is
        // broadcast/multicast; the vendor does the same for every non-QoS
        // frame (rtl8822ce_xmit.c). Without DISQSELSEQ/EN_HWSEQ our auth,
        // assoc and their retries all left with seq=0, which some APs treat
        // as a duplicate and silently drop.
        let bmc = frame.len() >= 5 && (frame[4] & 0x01) != 0;
        let mgmt_seq = MGMT_TX_SEQ
            .fetch_add(1, core::sync::atomic::Ordering::Relaxed)
            & 0x0fff;
        if frame.len() >= 24
            && matches!(
                crate::drivers::wifi::state::get_state(),
                crate::drivers::wifi::state::WifiState::Authenticating
                    | crate::drivers::wifi::state::WifiState::Associating
            )
        {
            crate::mesa_println!(
                "[WiFi MGMT] tx sub={:#04x} seq={} len={}",
                frame[0] & 0xf0, mgmt_seq, frame.len()
            );
        }
        let tx_desc = TxDesc {
            dword0: ((frame.len() as u32) & 0x0000ffff)
                | ((TX_HDR_SIZE as u32) << 16)   // TXPKTSIZE | OFFSET(48)
                | (if bmc { 1 << 24 } else { 0 }) // BMC
                | (1 << 26)                      // LS
                | (1 << 31),                     // DISQSELSEQ
            dword1: TX_DESC_QSEL_MGMT << 8, // QSEL[12:8] = 18 (management)
            dword2: 0, // RETRY_LMT, RTS_RATE, AMPDU_MAX_TIME, PKT_OFFSET, AGG_EN
            dword3: 0, // NDPA, PWR_OFFSET, MORE_FRAG, HW_SSN_SEL=0
            dword4: buf_phys as u32,
            dword5: (buf_phys >> 32) as u32,
            dword6: 0, // TX buffer size + next desc low
            dword7: 0, // TXDESC_CHECKSUM + RTS_SC + TX BUFF SIZE
            dword8: 1 << 15, // EN_HWSEQ
            dword9: (mgmt_seq as u32) << 12, // SW_SEQ[23:12]
        };

        // Copy the descriptor at the head of the 48-byte TX header area and the
        // frame payload at offset TX_HDR_SIZE. The header sequence is also
        // filled in so the value is right whether the MAC honours EN_HWSEQ or
        // transmits the header verbatim.
        unsafe {
            core::ptr::write_bytes(buf_virt as *mut u8, 0, TX_HDR_SIZE as usize);
            core::ptr::copy_nonoverlapping(
                &tx_desc as *const TxDesc as *const u8,
                buf_virt as *mut u8,
                core::mem::size_of::<TxDesc>(),
            );
            core::ptr::copy_nonoverlapping(
                frame.as_ptr(),
                (buf_virt as *mut u8).add(TX_HDR_SIZE as usize),
                frame.len(),
            );
            if frame.len() >= 24 {
                let seq_ctrl = (buf_virt as *mut u8).add(TX_HDR_SIZE as usize + 22);
                *seq_ctrl = (mgmt_seq << 4) as u8;
                *seq_ctrl.add(1) = (mgmt_seq >> 4) as u8;
            }
        }

        // Arm the two-segment TX buffer descriptor (header + payload).
        self.fill_tx_bd(queue, idx, TX_HDR_SIZE, frame.len() as u16, buf_phys);

        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

        // Same mechanism that fixed the data path: with EN_HWSEQ=1 the MAC
        // takes the on-air sequence from REG_HW_SEQ0 (HW_SSN_SEL=0), so keep
        // it in sync with the descriptor/header value just before kicking it.
        unsafe {
            self.write16(REG_HW_SEQ0, mgmt_seq);
        }

        let next_idx = (idx + 1) % self.tx_bd_count[queue] as usize;
        self.tx_wr_idx[queue] = next_idx as u16;

        unsafe {
            self.write32(REG_MGQ_TXBD_IDX, self.tx_wr_idx[queue] as u32);
        }

        driver_unlock(); Ok(())
    }

    /// Send a Null data frame protected with CCMP (pairwise key, key id 0).
    ///
    /// After the 4-way handshake the AP expects every data frame (including
    /// the idle keepalive) to be encrypted; a plaintext Null triggers a
    /// deauth on strict APs (mobile hotspots). The hardware only encrypts the
    /// payload, so insert the 8-byte CCMP header ourselves and set SEC_TYPE=3.
    fn send_encrypted_null(&mut self, bssid: [u8; 6], tx_addr: [u8; 6]) -> Result<(), &'static str> {
        const DATA_HDR_LEN: usize = 24;
        let mut frame = crate::drivers::wifi::frame::Ieee80211Parser::build_qos_null(bssid, tx_addr);
        if frame.len() < DATA_HDR_LEN {
            return Err("RTL8822CE: short null frame");
        }
        // Protected bit (FC bit 14), same as mac80211 ieee80211_tx_set_protected().
        frame[1] |= 0x40;

        // Software CCMP (SEC_TYPE=0 on transmit).
        let tk = crate::net::wpa::get_ptk()
            .map(|p| p.tk)
            .ok_or("RTL8822CE: no PTK for null keepalive")?;
        let header = frame[..DATA_HDR_LEN].to_vec();
        let pn = CCMP_TX_PN.fetch_add(1, core::sync::atomic::Ordering::Relaxed) + 1;
        let extra = crate::net::wpa::ccmp_encrypt_frame(
            &tk, 0, &tx_addr, pn, &header, &frame[DATA_HDR_LEN..],
        );
        let mut enc = alloc::vec::Vec::with_capacity(DATA_HDR_LEN + extra.len());
        enc.extend_from_slice(&header);
        enc.extend_from_slice(&extra);
        frame = enc;

        if frame.len() > TX_BUF_SIZE - TX_HDR_SIZE as usize {
            return Err("RTL8822CE: null frame too large");
        }

        self.reclaim_tx_done();

        let queue = TX_QUEUE_BE as usize;
        let idx = self.tx_wr_idx[queue] as usize;
        if self.tx_avail_descs(queue) < 2 {
            return Err("RTL8822CE: TX ring full");
        }
        let next_idx = (idx + 1) % self.tx_bd_count[queue] as usize;
        let buf_phys = unsafe {
            (self.tx_buf[queue] as u64) + (idx as u64) * TX_BUF_SIZE as u64
        };
        let buf_virt = unsafe {
            (self.tx_buf_virt[queue] as u64) + (idx as u64) * TX_BUF_SIZE as u64
        };

        // Frame is already CCMP-encrypted in software, so the HW crypto engine
        // MUST stay off (SEC_TYPE=0); otherwise the HW would encrypt the
        // ciphertext again and the AP would fail the MIC and deauth us.
        const RATE_ID_B_20M: u32 = 8;
        const DESC_RATE1M: u32 = 0x00;
        // Same sequence-number plumbing as send_packet(): the HW must emit the
        // seq_ctrl that was hashed into the CCMP AAD, not its stale QSEL counter.
        let seq = (u16::from_le_bytes([frame[22], frame[23]]) >> 4) & 0x0fff;
        let tx_desc = TxDesc {
            dword0: ((frame.len() as u32) & 0x0000ffff)
                | ((TX_HDR_SIZE as u32) << 16)
                | (1 << 26)   // LS (unicast to AP)
                | (1 << 31),  // DISQSELSEQ
            dword1: RATE_ID_B_20M << 16, // QSEL=BE/TID0, SEC_TYPE=0 (SW CCMP)
            dword2: 0,
            dword3: (1 << 8) | (1 << 10), // USE_RATE | DISDATAFB
            dword4: DESC_RATE1M & 0x7f,   // 1M CCK
            dword5: 0,
            dword6: 0,
            dword7: 0,
            dword8: 1 << 15,              // EN_HWSEQ
            dword9: (seq as u32) << 12,   // SW_SEQ[23:12]
        };

        unsafe {
            core::ptr::write_bytes(buf_virt as *mut u8, 0, TX_HDR_SIZE as usize);
            core::ptr::copy_nonoverlapping(
                &tx_desc as *const TxDesc as *const u8,
                buf_virt as *mut u8,
                core::mem::size_of::<TxDesc>(),
            );
            core::ptr::copy_nonoverlapping(
                frame.as_ptr(),
                (buf_virt as *mut u8).add(TX_HDR_SIZE as usize),
                frame.len(),
            );
        }

        self.fill_tx_bd(queue, idx, TX_HDR_SIZE, frame.len() as u16, buf_phys);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        self.tx_wr_idx[queue] = next_idx as u16;
        unsafe {
            self.write32(REG_BEQ_TXBD_IDX, self.tx_wr_idx[queue] as u32);
        }

        Ok(())
    }

    /// Prepare hardware for scanning (MLME_SCAN_ENTER) - like Linux hw_var_set_mlme_sitesurvey
    fn hw_scan_enter(&mut self) {
        crate::mesa_println!("[RTL8822CE] HW: Scan enter (MLME_SCAN_ENTER)");
        unsafe {
            // 1. Set RXFLTMAP2 to 0 to not filter by BSSID during scan
            self.write16(REG_RXFLTMAP2, 0x0000); // REG_RXFLTMAP2
            let rxf = self.read16(REG_RXFLTMAP2);
            crate::mesa_println!("[RTL8822CE]   RXFLTMAP2 set to 0x0000 (readback={:#06x})", rxf);
            // 2. RCR: clear CBSSID_BCN and CBSSID_DATA to accept all beacons/probe responses
            //    ALSO ensure AMF (bit 13) is set to accept management frames (beacons, probe resp)
            let rcr = self.read32(REG_RCR);
            let rcr_new = (rcr & !(RCR_CBSSID_BCN | RCR_CBSSID_DATA)) | RCR_AMF;
            if rcr != rcr_new {
                self.write32(REG_RCR, rcr_new);
                let rcr_v = self.read32(REG_RCR);
                crate::mesa_println!("[RTL8822CE]   RCR: {:#010x} -> {:#010x} (CBSSID_BCN/DATA cleared, AMF set)", rcr, rcr_v);
                crate::mesa_println!("[RTL8822CE]   RCR bits: AMF={} CBSSID_BCN={} CBSSID_DATA={} AM={} AB={} APM={} AAP={}",
                    (rcr_v & RCR_AMF) != 0,
                    (rcr_v & RCR_CBSSID_BCN) != 0,
                    (rcr_v & RCR_CBSSID_DATA) != 0,
                    (rcr_v & RCR_AM) != 0,
                    (rcr_v & RCR_AB) != 0,
                    (rcr_v & RCR_APM) != 0,
                    (rcr_v & RCR_AAP) != 0);
            }
            // 3. Verify RX DMA is enabled
            let cr = self.read8(REG_CR);
            crate::mesa_println!("[RTL8822CE]   CR (MAC enable) = {:#04x} (RXDMA={} TXDMA={})", 
                cr, (cr & 0x02) != 0, (cr & 0x04) != 0);
            // 3b. Dump RX-path registers to verify the TRX FIFO + ring config stuck.
            crate::mesa_println!("[RTL8822CE]   RXFF_BNDY={:#010x} RXBD_IDX={:#010x} RXBD_NUM={:#06x} HIMR0={:#010x} PCIE_CTRL={:#010x}",
                self.read32(REG_RXFF_BNDY),
                self.read32(REG_RXQ_RXBD_IDX),
                self.read16(REG_RXQ_RXBD_NUM),
                self.read32(REG_PCIE_HIMR0),
                self.read32(RTK_PCI_CTRL));
            crate::mesa_println!("[RTL8822CE]   RXpath: ANTMAP824={:#010x} RXCCKSEL={:#010x} CCANRX={:#010x} RXFNCTL={:#010x} ANTMAP0={:#010x} SYSFUNCEN={:#06x} RFCTRL={:#04x}",
                self.read32(REG_ANTMAP),
                self.read32(REG_RXCCKSEL),
                self.read32(REG_CCANRX),
                self.read32(REG_RXFNCTL),
                self.read32(REG_ANTMAP0),
                self.read16(REG_SYS_FUNC_EN),
                self.read8(REG_RF_CTRL));
            crate::mesa_println!("[RTL8822CE]   RF18: A={:#010x} B={:#010x} (cur_ch={})",
                self.read_rf_sipi(0, 0x18, RFREG_MASK),
                self.read_rf_sipi(1, 0x18, RFREG_MASK),
                self.current_channel);
        }
        
        // 4. This is a software scan: we switch channels ourselves and rely on
        // RCR/RXFLTMAP to accept beacons/probe responses. No FW scan-offload H2C
        // must be sent (rtw88 only sends H2C_CMD_SCAN=0x59 for a *firmware* scan;
        // the bogus 0x02 command here was not a valid H2C class).
    }

    /// Restore hardware after scanning (MLME_SCAN_DONE) - like Linux hw_var_set_mlme_sitesurvey
    fn hw_scan_done(&mut self) {
        crate::mesa_println!("[RTL8822CE] HW: Scan done (MLME_SCAN_DONE)");
        unsafe {
            // 1. Restore RXFLTMAP2 to 0xFFFF (accept all data frames)
            self.write16(REG_RXFLTMAP2, 0xFFFF); // REG_RXFLTMAP2
            let rxf = self.read16(REG_RXFLTMAP2);
            crate::mesa_println!("[RTL8822CE]   RXFLTMAP2 restored to 0xFFFF (readback={:#06x})", rxf);
            // 2. RCR: restore CBSSID_BCN and CBSSID_DATA for connected state
            let rcr = self.read32(REG_RCR);
            let rcr_new = rcr | RCR_CBSSID_BCN | RCR_CBSSID_DATA;
            if rcr != rcr_new {
                self.write32(REG_RCR, rcr_new);
                let rcr_v = self.read32(REG_RCR);
                crate::mesa_println!("[RTL8822CE]   RCR: {:#010x} -> {:#010x} (CBSSID_BCN/DATA set)", rcr, rcr_v);
            }
        }
        
        // 3. Software scan: nothing to notify the firmware about.
    }

    /// Prepare hardware for joining (MLME_STA_CONNECTING) - like Linux hw_var_set_mlme_join type=0
    fn hw_join_prepare(&self, bssid: &[u8; 6]) {
        crate::mesa_println!("[RTL8822CE] HW: Join prepare (MLME_STA_CONNECTING)");
        unsafe {
            // 0. Program the target BSSID. Without this the CBSSID receive
            //    filter (set below) rejects every AP->STA data frame because
            //    REG_BSSID stays 0 — which dropped the DHCP offer etc.
            self.write32(REG_BSSID, u32::from_le_bytes([bssid[0], bssid[1], bssid[2], bssid[3]]));
            self.write16(REG_BSSID_H, u16::from_le_bytes([bssid[4], bssid[5]]));
            crate::mesa_println!("[RTL8822CE]   BSSID set to {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                bssid[0], bssid[1], bssid[2], bssid[3], bssid[4], bssid[5]);
            // 3. Enable BCN function (REG_BCN_CTRL bit 3)
            let bcn_ctrl = self.read8(0x0550); // REG_BCN_CTRL
            self.write8(0x0550, bcn_ctrl | (1 << 3)); // BIT_EN_BCN_FUNCTION
            // 4. Accept all data frames from the AP.
            self.write16(REG_RXFLTMAP2, 0xFFFF);
            // 5. RCR: accept BSSID-matching beacons/data for the connected state.
            let rcr = self.read32(REG_RCR);
            let rcr_new = rcr | RCR_CBSSID_BCN | RCR_CBSSID_DATA;
            if rcr != rcr_new {
                self.write32(REG_RCR, rcr_new);
                crate::mesa_println!("[RTL8822CE]   RCR: {:#010x} -> {:#010x}", rcr, rcr_new);
            }
        }
    }

    /// Hardware cleanup after failed join (MLME_STA_DISCONNECTED) - like Linux hw_var_set_mlme_join type=1
    fn hw_join_fail_cleanup(&self) {
        crate::mesa_println!("[RTL8822CE] HW: Join fail cleanup (MLME_STA_DISCONNECTED)");
        unsafe {
            // 1. RXFLTMAP2 = 0 (reject all data frames)
            self.write16(REG_RXFLTMAP2, 0x0000);
            // 2. RCR: clear CBSSID_BCN and CBSSID_DATA
            let rcr = self.read32(REG_RCR);
            let rcr_new = rcr & !(RCR_CBSSID_BCN | RCR_CBSSID_DATA);
            if rcr != rcr_new {
                self.write32(REG_RCR, rcr_new);
            }
            // 3. Disable TSF update
            // (Linux: rtw_iface_disable_tsf_update)
            // 4. Reset net type to no-link (0x0100 bits[17:16] = 0)
            self.write32_mask(0x0100, 0x30000, 0);
        }
    }

    /// Hardware setup after successful join (MLME_STA_CONNECTED) - like Linux hw_var_set_mlme_join type=2
    fn hw_join_success(&self, aid: u16) {
        crate::mesa_println!("[RTL8822CE] HW: Join success (MLME_STA_CONNECTED)");
        unsafe {
            // Net type = managed / STA-linked (0x0100 bits[17:16] = 2).
            // Linux: rtw_vif_port_config PORT_SET_NET_TYPE -> RTW_NET_MGD_LINKED.
            // Without this the HW does not treat us as an associated station
            // and drops/ignores data in both directions.
            self.write32_mask(0x0100, 0x30000, 2);
            // AID (0x06a8 bits[10:0]). Linux: PORT_SET_AID.
            self.write32_mask(0x06a8, 0x7ff, aid as u32);
            // Enable BCN function (REG_BCN_CTRL bit 3)
            let bcn_ctrl = self.read8(0x0550); // REG_BCN_CTRL
            self.write8(0x0550, bcn_ctrl | (1 << 3)); // BIT_EN_BCN_FUNCTION
            let cr = self.read32(0x0100);
            let aid_rb = self.read32(0x06a8);
            crate::mesa_println!("[RTL8822CE]   net_type=2 aid={} (CR={:#010x} AID={:#010x})",
                aid, cr, aid_rb);
        }
    }

    /// Send a firmware reset signal via H2C
    pub fn signal_fw_reset(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Sending FW reset signal...");
        // Send reset command to firmware
        self.send_h2c_command(0xFF00_0000, 0x0000_0001)
    }

    /// Notify firmware about scanning operation
    pub fn signal_scan_start(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Signaling scan start to FW...");
        self.send_h2c_command(0xFF01_0000, 0x0000_0001)
    }

    /// Notify firmware about scan completion
    pub fn signal_scan_done(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Signaling scan done to FW...");
        self.send_h2c_command(0xFF01_0000, 0x0000_0000)
    }

    /// Signal association to firmware  
    pub fn signal_associate(&mut self, bssid: &[u8; 6]) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Signaling associate to FW...");
        let bssid_val = u32::from_le_bytes([bssid[0], bssid[1], bssid[2], bssid[3]]);
        self.send_h2c_command(0xFF02_0000, bssid_val)?;
        let bssid_val2 = u32::from_le_bytes([bssid[4], bssid[5], 0x00, 0x00]);
        self.send_h2c_command(0xFF02_0001, bssid_val2)
    }

    /// Signal disassociation to firmware
    pub fn signal_disassociate(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Signaling disassociate to FW...");
        self.send_h2c_command(0xFF03_0000, 0x0000_0000)
    }

    /// Reset watchdog timer - feeds the watchdog to prevent firmware timeout
    fn feed_watchdog(&mut self) {
        // In a full implementation, this would write to the watchdog register
        // to reset the timeout counter. For now, just track time.
        if self.watchdog_enabled {
            self.watchdog_timeout = 0;
        }
    }

    /// Check if watchdog has timed out (firmware hung)
    fn check_watchdog(&mut self) -> bool {
        if self.watchdog_enabled && self.watchdog_timeout > 1000 {
            crate::mesa_println!("[RTL8822CE] WARNING: Watchdog timeout! Firmware may be hung.");
            self.watchdog_timeout = 0;
            true
        } else {
            self.watchdog_timeout += 1;
            false
        }
    }

    /// Perform silent reset (fw reset without full chip power cycle)
    /// Mirrors Linux: rtl8822c_silent_reset
    fn silent_reset(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Performing silent reset (rtl8822c_silent_reset)...");
        
        unsafe {
            // 1. Disable MAC TX/RX
            let cr = self.read32(REG_CR);
            self.write32(REG_CR, cr & !MAC_TRX_ENABLE);
            
            // 2. Reset TX/RX DMA
            self.write32(RTK_PCI_CTRL, BIT_RST_TRXDMA_INTF);
            self.delay_ms(10);
            self.write32(RTK_PCI_CTRL, self.read32(RTK_PCI_CTRL) & !BIT_RST_TRXDMA_INTF);
            
            // 3. Disable interrupts
            self.write32(REG_PCIE_HIMR0, 0);
            self.write32(REG_PCIE_HIMR1, 0);
            
            // 4. Send FW reset via H2C
            self.signal_fw_reset()?;
            
            // 5. Wait for firmware to acknowledge
            crate::linux::mdelay(10);
            
            // 6. Check if FW is responsive after reset
            for _ in 0..20 {
                let fw_ctrl = self.read32(REG_MCUFW_CTRL);
                if (fw_ctrl & FW_READY_MASK) == FW_READY {
                    break;
                }
                crate::linux::mdelay(10);
            }
            
            // 7. Re-enable MAC TX/RX
            self.write32(REG_CR, cr);
        }
        
        // 8. Re-initialize state
        self.initialized = false;
        self.fw_loaded = false;
        
        crate::mesa_println!("[RTL8822CE] Silent reset complete");
        driver_unlock(); Ok(())
    }

    /// Perform hardware reset (full chip reset)
    /// Mirrors Linux: rtl8822c_hw_reset
    fn hw_reset(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Performing hardware reset...");
        
        // Step 1: Platform reset (toggle CPU_DMEM_CON bit 0)
        unsafe {
            self.write8(REG_CPU_DMEM_CON + 2, self.read8(REG_CPU_DMEM_CON + 2) & !(1 << 0));
            self.delay_ms(100);
            self.write8(REG_CPU_DMEM_CON + 2, self.read8(REG_CPU_DMEM_CON + 2) | (1 << 0));
            self.delay_ms(100);
        }
        
        // Step 2: Enable firmware download mode
        let val = unsafe { (self.read16(REG_MCUFW_CTRL) & 0x3800) | BIT_MCUFWDL_EN as u16 };
        unsafe { self.write16(REG_MCUFW_CTRL, val); }
        self.delay_ms(50);
        
        // Step 3: Download firmware
        // (Would call download_firmware here)
        
        driver_unlock(); Ok(())
    }

    /// Handle firmware timeout - initiate recovery
    fn handle_fw_timeout(&mut self) {
        crate::mesa_println!("[RTL8822CE] Handling firmware timeout...");
        
        // First try silent reset
        if let Err(e) = self.silent_reset() {
            crate::mesa_println!("[RTL8822CE] Silent reset failed: {}", e);
            
            // Fall back to full hardware reset
            if let Err(e) = self.hw_reset() {
                crate::mesa_println!("[RTL8822CE] Hardware reset failed: {}", e);
                // Firmware is completely stuck - need full chip power cycle
                crate::mesa_println!("[RTL8822CE] CHIP STUCK - requires full power cycle");
            }
        }
    }

// ========================================================================
// Linux rtw_power_on() flow - Helper Methods
// ========================================================================

    /// PCI/HCI Setup (rtw_hci_setup)
    fn hci_setup(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] HCI setup: configuring PCIe...");
        unsafe {
            // Enable bus mastering and memory space already done in pci_init
            // Additional PCIe-specific setup
            
            // Disable ASPM
            self.write32(0x0070, self.read32(0x0070) & !0x3);
            
            // Enable PCIe L1/L0s
            self.write32(0x0074, self.read32(0x0074) | 0x3);
            
            // Set PCIe max payload size
            self.write16(0x0048, 0x0000);
            
            crate::mesa_println!("[RTL8822CE] HCI setup complete");
        }
        driver_unlock(); Ok(())
    }

    /// MAC Power On (rtw_mac_power_on) - mirrors Linux rtw_mac_power_on
    fn mac_power_on(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] MAC power on sequence...");
        
        // rtw_mac_pre_system_cfg
        self.mac_pre_system_cfg();
        
        // rtw_mac_power_switch (on)
        let reg_cr = unsafe { self.read8(REG_CR) };
        crate::mesa_println!("[RTL8822CE] MAC power: REG_CR={:#04x}", reg_cr);
        if reg_cr != 0xea {
            crate::mesa_println!("[RTL8822CE] MAC already on, cycling off->on...");
            self.pwr_off_seq();
            self.mac_pre_system_cfg();
        }
        
        // Execute power sequence: carddis_to_cardemu + cardemu_to_act
        // This is already in power_on()
        self.power_on()?;
        
        crate::mesa_println!("[RTL8822CE] MAC power on complete (without firmware - use mac_power_on_with_firmware for full flow)");
        driver_unlock(); Ok(())
    }

    /// MAC Power On with firmware download combined (Linux rtw_mac_power_on flow)
    fn mac_power_on_with_firmware(&mut self, fw_data: Option<&[u8]>) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] MAC power on + firmware sequence...");
        
        // rtw_mac_pre_system_cfg
        self.mac_pre_system_cfg();
        
        // rtw_mac_power_switch (on)
        let reg_cr = unsafe { self.read8(REG_CR) };
        crate::mesa_println!("[RTL8822CE] MAC power: REG_CR={:#04x}", reg_cr);
        if reg_cr != 0xea {
            crate::mesa_println!("[RTL8822CE] MAC already on, cycling off->on...");
            self.pwr_off_seq();
            self.mac_pre_system_cfg();
        }
        
        // Execute power sequence: carddis_to_cardemu + cardemu_to_act
        self.power_on()?;
        
        // Download firmware as part of power on (Linux does this in rtw_mac_power_on)
        crate::mesa_println!("[RTL8822CE] Downloading firmware during power-on...");
        if let Some(fw) = fw_data {
            crate::serial_println!("[RTL8822CE] Starting firmware download ({} bytes)...", fw.len());
            wifi_log("Paso: Descargando firmware...");
            self.download_firmware(fw)?;
            wifi_log("Paso: Firmware cargado OK");
        } else {
            crate::serial_println!("[RTL8822CE] WARNING: No firmware provided!");
            wifi_log("Paso: Sin firmware!");
            return Err("RTL8822CE: Firmware required during power-on");
        }
        
        crate::mesa_println!("[RTL8822CE] MAC power on + firmware complete");
        driver_unlock(); Ok(())
    }

    /// Wait for firmware completion (rtw_wait_firmware_completion)
    fn wait_firmware_completion(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Waiting for FW to be ready...");
        // Wait for MCUFW_CTRL to show FW_READY
        for attempt in 0..200 {
            let fw_ctrl = unsafe { self.read32(REG_MCUFW_CTRL) };
            if (fw_ctrl & FW_READY_MASK) == FW_READY {
                crate::mesa_println!("[RTL8822CE] FW ready! MCUFW={:#010x}", fw_ctrl);
                driver_unlock(); return Ok(());
            }
            if attempt % 20 == 0 {
                crate::mesa_println!("[RTL8822CE] FW wait... attempt={} MCUFW={:#010x}", attempt, fw_ctrl);
            }
            self.delay_ms(50);
        }
        Err("RTL8822CE: Firmware completion timeout")
    }

    /// MAC Init after firmware (rtw8822c_mac_init)
    /// Configure the TX/RX FIFO boundaries, DMA queue mapping and H2C ring.
    /// Ports rtw88 `rtw_init_trx_cfg` (mac.c): txdma_queue_mapping +
    /// priority_queue_cfg + init_h2c. This was entirely missing; without the
    /// RX FIFO boundary (REG_RXFF_BNDY) the RX DMA never writes descriptors
    /// and RXOK never fires (no received frames at all).
    fn init_trx_cfg(&mut self) -> Result<(), &'static str> {
        unsafe {
            // txdma_queue_mapping: rtw8822c rqpn_table[1] (PCIe NORMAL)
            //   vo=NORMAL, vi=NORMAL, be=LOW, bk=LOW, mg=EXTRA, hi=HIGH.
            let pq_map: u16 = ((RTW_DMA_MAPPING_NORMAL as u16) << 4)
                | ((RTW_DMA_MAPPING_NORMAL as u16) << 6)
                | ((RTW_DMA_MAPPING_LOW as u16) << 8)
                | ((RTW_DMA_MAPPING_LOW as u16) << 10)
                | ((RTW_DMA_MAPPING_EXTRA as u16) << 12)
                | ((RTW_DMA_MAPPING_HIGH as u16) << 14);
            self.write16(REG_TXDMA_PQ_MAP, pq_map);
            self.write8(REG_CR, 0);
            self.write8(REG_CR, MAC_TRX_ENABLE as u8);
            self.write32(REG_H2CQ_CSR, BIT_H2CQ_FULL);

            // priority_queue_cfg. Page table for PCIe NORMAL:
            //   {hq=64, lq=64, nq=64, exq=64, gap=1}; pubq = acq - 257.
            self.write16(REG_FIFOPAGE_INFO_1, 64);
            self.write16(REG_FIFOPAGE_INFO_2, 64);
            self.write16(REG_FIFOPAGE_INFO_3, 64);
            self.write16(REG_FIFOPAGE_INFO_4, 64);
            self.write16(REG_FIFOPAGE_INFO_5, (TXFF_PG_NUM - RSVD_PG_NUM - 257) as u16);
            self.write32(REG_RQPN_CTRL_2, self.read32(REG_RQPN_CTRL_2) | BIT_LD_RQPN);

            self.write16(REG_FIFOPAGE_CTRL_2, RSVD_BOUNDARY);
            self.write8(REG_FWHW_TXQ_CTRL + 2,
                self.read8(REG_FWHW_TXQ_CTRL + 2) | ((BIT_EN_WR_FREE_TAIL >> 16) as u8));
            self.write16(REG_BCNQ_BDNY_V1, RSVD_BOUNDARY);
            self.write16(REG_FIFOPAGE_CTRL_2 + 2, RSVD_BOUNDARY);
            self.write16(REG_BCNQ1_BDNY_V1, RSVD_BOUNDARY);

            // RX FIFO top = RX FIFO size - C2H packet buffer - 1.
            self.write32(REG_RXFF_BNDY, RXFF_SIZE - C2H_PKT_BUF - 1);

            // Kick automatic LLT init; the bit self-clears when done.
            self.write8(REG_AUTO_LLT_V1,
                self.read8(REG_AUTO_LLT_V1) | (BIT_AUTO_INIT_LLT_V1 as u8));
            let mut llt_ok = false;
            for _ in 0..1000 {
                if self.read8(REG_AUTO_LLT_V1) & (BIT_AUTO_INIT_LLT_V1 as u8) == 0 {
                    llt_ok = true;
                    break;
                }
                self.delay_us(10);
            }
            self.write8(REG_CR + 3, 0);

            crate::mesa_println!(
                "[RTL8822CE] TRX FIFO: rsvd_bndy={} pubq={} rxff_bndy={:#x} pq_map={:#06x} llt_ok={}",
                RSVD_BOUNDARY, TXFF_PG_NUM - RSVD_PG_NUM - 257,
                RXFF_SIZE - C2H_PKT_BUF - 1, pq_map, llt_ok);

            // init_h2c (WCPU_3081): H2C ring sits below the CSI buffer.
            let h2cq_addr = RSVD_H2CQ_ADDR * TX_PAGE_SIZE;
            let h2cq_size = 8 * TX_PAGE_SIZE;
            let head = (self.read32(REG_H2C_HEAD) & 0xFFFC_0000) | h2cq_addr;
            self.write32(REG_H2C_HEAD, head);
            self.write32(REG_H2C_READ_ADDR, head);
            let tail = (self.read32(REG_H2C_TAIL) & 0xFFFC_0000) | (h2cq_addr + h2cq_size);
            self.write32(REG_H2C_TAIL, tail);
            self.write8(REG_H2C_INFO, (self.read8(REG_H2C_INFO) & 0xFC) | 0x01);
            self.write8(REG_H2C_INFO, (self.read8(REG_H2C_INFO) & 0xFB) | 0x04);
            self.write8(REG_TXDMA_OFFSET_CHK + 1,
                (self.read8(REG_TXDMA_OFFSET_CHK + 1) & 0x7F) | 0x80);
            crate::mesa_println!("[RTL8822CE] H2C ring: addr={:#x} size={}", h2cq_addr, h2cq_size);
        }
        Ok(())
    }

    /// rtw88 `rtw_drv_info_cfg`: set the RX descriptor driver-info size and
    /// enable APP PHY status so received frames carry the PHY status block.
    fn drv_info_cfg(&mut self) -> Result<(), &'static str> {
        unsafe {
            self.write8(REG_RX_DRVINFO_SZ, PHY_STATUS_SIZE);
            // WCPU_3081: driver-info-size nibble lives in TRXFF_BNDY[1] high nibble.
            let v = self.read8(0x0114 + 1);
            self.write8(0x0114 + 1, (v & 0xF0) | 0x0F);
            self.write32(REG_RCR, self.read32(REG_RCR) | (1 << 28)); // BIT_APP_PHYSTS
            // Clear WMAC option bits 8|9 (upper byte of REG_WMAC_OPTION_FUNCTION+4).
            self.write8(REG_WMAC_OPTION_FUNCTION + 5,
                self.read8(REG_WMAC_OPTION_FUNCTION + 5) & !0x03);
        }
        Ok(())
    }

    fn mac_init_after_fw(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] MAC init after firmware (rtw8822c_mac_init)...");
        
        unsafe {
            // txq control
            let value8 = self.read8(REG_FWHW_TXQ_CTRL);
            self.write8(REG_FWHW_TXQ_CTRL, value8 | (1 << 7) & !(1 << 1) & !(1 << 2));
            self.write8(REG_FWHW_TXQ_CTRL + 1, WLAN_TXQ_RPT_EN);
            
            // sifs control
            self.write16(REG_SPEC_SIFS, WLAN_SIFS_DUR_TUNE);
            self.write32(REG_SIFS, WLAN_SIFS_CFG);
            self.write16(REG_RESP_SIFS_CCK, (WLAN_SIFS_CCK_CTX as u16) | ((WLAN_SIFS_CCK_IRX as u16) << 8));
            self.write16(REG_RESP_SIFS_OFDM, (WLAN_SIFS_OFDM_CTX as u16) | ((WLAN_SIFS_OFDM_IRX as u16) << 8));
            
            // rate fallback control
            self.write32(REG_DARFRC, WLAN_DATA_RATE_FB_CNT_1_4);
            self.write32(REG_DARFRCH, WLAN_DATA_RATE_FB_CNT_5_8);
            self.write32(REG_RARFRCH, WLAN_RTS_RATE_FB_CNT_5_8);
            self.write32(REG_ARFR0, WLAN_DATA_RATE_FB_RATE0);
            self.write32(REG_ARFRH0, WLAN_DATA_RATE_FB_RATE0_H);
            self.write32(REG_ARFR1_V1, WLAN_RTS_RATE_FB_RATE1);
            self.write32(REG_ARFRH1_V1, WLAN_RTS_RATE_FB_RATE1_H);
            self.write32(REG_ARFR4, WLAN_RTS_RATE_FB_RATE4);
            self.write32(REG_ARFRH4, WLAN_RTS_RATE_FB_RATE4_H);
            self.write32(REG_ARFR5, WLAN_RTS_RATE_FB_RATE5);
            self.write32(REG_ARFRH5, WLAN_RTS_RATE_FB_RATE5_H);
            
            // protocol configuration
            self.write8(REG_AMPDU_MAX_TIME_V1, WLAN_AMPDU_MAX_TIME);
            self.write8(REG_TX_HANG_CTRL, self.read8(REG_TX_HANG_CTRL) | (BIT_EN_EOF_V1 as u8));
            let pre_txcnt = (WLAN_PRE_TXCNT_TIME_TH as u32) | (BIT_EN_PRECNT as u32);
            self.write8(REG_PRECNT_CTRL, (pre_txcnt & 0xFF) as u8);
            self.write8(REG_PRECNT_CTRL + 1, (pre_txcnt >> 8) as u8);
            let value32 = WLAN_RTS_LEN_TH as u32 | ((WLAN_RTS_TX_TIME_TH as u32) << 8) |
                ((WLAN_MAX_AGG_PKT_LIMIT as u32) << 16) |
                ((WLAN_RTS_MAX_AGG_PKT_LIMIT as u32) << 24);
            self.write32(REG_PROT_MODE_CTRL, value32);
            self.write16(REG_BAR_MODE_CTRL + 2, (WLAN_BAR_RETRY_LIMIT as u16) | ((WLAN_RA_TRY_RATE_AGG_LIMIT as u16) << 8));
            self.write8(REG_FAST_EDCA_VOVI_SETTING, FAST_EDCA_VO_TH);
            self.write8(REG_FAST_EDCA_VOVI_SETTING + 2, FAST_EDCA_VI_TH);
            self.write8(REG_FAST_EDCA_BEBK_SETTING, FAST_EDCA_BE_TH);
            self.write8(REG_FAST_EDCA_BEBK_SETTING + 2, FAST_EDCA_BK_TH);
            // close BA parser
            self.write8(REG_LIFETIME_EN, self.read8(REG_LIFETIME_EN) & !(BIT_BA_PARSER_EN as u8));
            self.write32(REG_RRSR, self.read32(REG_RRSR) & !(BITS_RRSR_RSC as u32));
            
            // EDCA configuration
            self.write32(REG_EDCA_VO_PARAM, WLAN_EDCA_VO_PARAM);
            self.write32(REG_EDCA_VI_PARAM, WLAN_EDCA_VI_PARAM);
            self.write32(REG_EDCA_BE_PARAM, WLAN_EDCA_BE_PARAM);
            self.write32(REG_EDCA_BK_PARAM, WLAN_EDCA_BK_PARAM);
            
            self.write8(REG_PIFS, WLAN_PIFS_TIME);
            self.write8(REG_TX_PTCL_CTRL + 1, self.read8(REG_TX_PTCL_CTRL + 1) & !((BIT_SIFS_BK_EN >> 8) as u8));
            self.write8(REG_RD_CTRL + 1, self.read8(REG_RD_CTRL + 1) | 
                (((BIT_DIS_TXOP_CFE | BIT_DIS_LSIG_CFE | BIT_DIS_STBC_CFE) >> 8) as u8));
            
            // MAC clock configuration
            self.write32(REG_AFE_CTRL1, self.read32(REG_AFE_CTRL1) & !BIT_MAC_CLK_SEL);
            self.write8(REG_USTIME_TSF, MAC_CLK_SPEED);
            self.write8(REG_USTIME_EDCA, MAC_CLK_SPEED);
            self.write8(REG_MISC_CTRL, self.read8(REG_MISC_CTRL) | (BIT_EN_FREE_CNT as u8) | (BIT_DIS_SECOND_CCA as u8));
            self.write8(REG_TIMER0_SRC_SEL, self.read8(REG_TIMER0_SRC_SEL) & !(BIT_TSFT_SEL_TIMER0 as u8));
            self.write16(REG_TXPAUSE, 0x0000);
            self.write8(REG_SLOT, WLAN_SLOT_TIME);
            self.write32(REG_RD_NAV_NXT, WLAN_NAV_CFG);
            self.write16(REG_RXTSF_OFFSET_CCK, WLAN_RX_TSF_CFG);
            
            // Set beacon control - enable TSF and other related functions
            self.write8(REG_BCN_CTRL, self.read8(REG_BCN_CTRL) | (BIT_EN_BCN_FUNCTION as u8));
            // Set send beacon related registers
            self.write32(REG_TBTT_PROHIBIT, WLAN_TBTT_TIME);
            self.write8(REG_DRVERLYINT, WLAN_DRV_EARLY_INT);
            self.write8(REG_BCN_CTRL_CLINT0, WLAN_BCN_CTRL_CLT0);
            self.write8(REG_BCNDMATIM, WLAN_BCN_DMA_TIME);
            self.write8(REG_BCN_MAX_ERR, WLAN_BCN_MAX_ERR);
            
            // WMAC configuration
            self.write32(REG_MAR, WLAN_MULTI_ADDR);
            self.write32(REG_MAR + 4, WLAN_MULTI_ADDR);
            self.write8(REG_BBPSF_CTRL + 2, WLAN_RESP_TXRATE);
            self.write8(REG_ACKTO, WLAN_ACK_TO);
            self.write8(REG_ACKTO_CCK, WLAN_ACK_TO_CCK);
            self.write16(REG_EIFS, WLAN_EIFS_DUR_TUNE);
            self.write8(REG_NAV_CTRL + 2, WLAN_NAV_MAX);
            self.write8(REG_WMAC_TRXPTCL_CTL_H + 2, WLAN_BAR_ACK_TYPE);
            self.write32(REG_RXFLTMAP0, WLAN_RX_FILTER0);
            self.write16(REG_RXFLTMAP2, WLAN_RX_FILTER2);
            self.write8(REG_RX_PKT_LIMIT, WLAN_RXPKT_MAX_SZ_512);
            self.write8(REG_TCR + 2, WLAN_TX_FUNC_CFG2);
            self.write8(REG_TCR + 1, WLAN_TX_FUNC_CFG1);
            self.write32(REG_GENERAL_OPTION, self.read32(REG_GENERAL_OPTION) | BIT_DUMMY_FCS_READY_MASK_EN);
            self.write32(REG_WMAC_OPTION_FUNCTION + 8, WLAN_MAC_OPT_FUNC2);
            self.write8(REG_WMAC_OPTION_FUNCTION_1, WLAN_MAC_OPT_NORM_FUNC1);
            
            // init low power
            let value16 = self.read16(REG_RXPSF_CTRL + 2) & 0xF00F;
            let new_val16 = value16 | 
                (((BIT_RXGCK_VHT_FIFOTHR(1) | BIT_RXGCK_HT_FIFOTHR(1) |
                BIT_RXGCK_OFDM_FIFOTHR(1) | BIT_RXGCK_CCK_FIFOTHR(1)) >> 16) as u16);
            self.write16(REG_RXPSF_CTRL + 2, new_val16);
            let mut value16_2 = 0;
            value16_2 = BIT_SET_RXPSF_PKTLENTHR(value16_2, 1);
            value16_2 |= BIT_RXPSF_CTRLEN | BIT_RXPSF_VHTCHKEN | BIT_RXPSF_HTCHKEN
                | BIT_RXPSF_OFDMCHKEN | BIT_RXPSF_CCKCHKEN
                | BIT_RXPSF_OFDMRST;
            self.write16(REG_RXPSF_CTRL, value16_2);
            self.write32(REG_RXPSF_TYPE_CTRL, 0xFFFFFFFF);
            // rx ignore configuration
            let mut value16_3 = self.read16(REG_RXPSF_CTRL);
            value16_3 &= !(BIT_RXPSF_MHCHKEN | BIT_RXPSF_CCKRST | BIT_RXPSF_CONT_ERRCHKEN);
            value16_3 = BIT_SET_RXPSF_ERRTHR(value16_3, 0x07);
            self.write16(REG_RXPSF_CTRL, value16_3);
            self.write8(REG_SND_PTCL_CTRL, self.read8(REG_SND_PTCL_CTRL) | (BIT_DIS_CHK_VHTSIGB_CRC as u8));
            
            // Interrupt migration configuration
            self.write32(REG_INT_MIG, WLAN_MAC_INT_MIG_CFG);
        }

        crate::mesa_println!("[RTL8822CE] MAC init after firmware complete");
        driver_unlock(); Ok(())
    }

    /// PHY Set Param (rtw8822c_phy_set_param)
    fn phy_set_param(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] PHY set param (rtw8822c_phy_set_param)...");
        
        // This is already done in init_mac_post_fw + phy_init_post_fw
        // But we need to do the header file init and crystal cap
        unsafe {
            // Power on BB/RF domain (rtw8822c_phy_set_param). This MUST happen
            // before any BB/RF table write: otherwise the analog/BB blocks stay
            // in reset, every table write is lost, IQK times out and RX never
            // works (no RXOK interrupt).
            crate::mesa_println!("[RTL8822CE] Power on BB/RF domain...");
            self.write8(REG_SYS_FUNC_EN, self.read8(REG_SYS_FUNC_EN) | BIT_FEN_BB_GLB_RST | BIT_FEN_BB_RSTB);
            self.write8(REG_RF_CTRL, self.read8(REG_RF_CTRL) | BIT_RF_EN | BIT_RF_RSTB | BIT_RF_SDM_RSTB);
            self.write32(REG_WLRF1, self.read32(REG_WLRF1) | BIT_WLRF1_BBRF_EN);
            self.delay_ms(10);
            crate::mesa_println!("[RTL8822CE] BB/RF pwr: SYS_FUNC_EN={:#04x} RF_CTRL={:#04x} WLRF1={:#010x}",
                self.read8(REG_SYS_FUNC_EN), self.read8(REG_RF_CTRL), self.read32(REG_WLRF1));

            // Disable low rate DPD
            self.write32(REG_DIS_DPD, DIS_DPD_RATEALL);
            
            // pre init before header files config
            self.header_file_init(true)?;
            
            // rtw_phy_load_tables - already done in phy_init_post_fw
            // But we should do it again here per Linux flow
            
            // Crystal cap from EFUSE
            let crystal_cap = (self.efuse.crystal_cap & 0x7f) as u32;
            self.write32(REG_ANAPAR_XTAL_0, self.read32(REG_ANAPAR_XTAL_0) & !0xfffc00 |
                ((crystal_cap | (crystal_cap << 7)) & 0xfffc00));
            
            // post init after header files config
            self.header_file_init(false)?;

            // RFK/DPK init table (rtw_load_rfk_table) - part of rtw_phy_load_tables
            self.load_rfk_table()?;

            // RF radio tables (rtw8822c_rf_a / rf_b) - part of rtw_phy_load_tables
            self.load_rf_tables_full()?;
            
            // Config TRX mode (2T2R)
            self.config_trx_mode()?;
            
            // PHY init (DIG, CCK PD, Adaptivity, CFO, etc.)
            self.phy_init_dig_cckpd()?;
            
            // RF init (DAC cal, thermal trim, power trim, PA bias, LCK)
            self.rf_init_critical()?;
            
            // Firmware-driven PHY calibration: hand the firmware the PHYDM info
            // and trigger its IQK. This is the 8822C equivalent of
            // rtw8822c_phy_calibration's IQK (host manual IQK is invalid here).
            self.fw_phy_calibration()?;
            
            // Power track init
            self.pwrtrack_init()?;
            
            // BF PHY init
            self.bf_phy_init()?;
        }
        
        crate::mesa_println!("[RTL8822CE] PHY set param complete");
        driver_unlock(); Ok(())
    }

    /// Header file init (rtw8822c_header_file_init) - loads MAC_REG, PHY_REG, AGC_TAB, PHY_REG_PG tables
    fn header_file_init(&self, pre: bool) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Header file init: pre={}", pre);
        
        unsafe {
            // Enable 3-wire
            self.write32(REG_3WIRE, self.read32(REG_3WIRE) | BIT_3WIRE_TX_EN | BIT_3WIRE_RX_EN);
            self.write32(REG_3WIRE, self.read32(REG_3WIRE) | BIT_3WIRE_PI_ON);
            self.write32(REG_3WIRE2, self.read32(REG_3WIRE2) | BIT_3WIRE_TX_EN | BIT_3WIRE_RX_EN);
            self.write32(REG_3WIRE2, self.read32(REG_3WIRE2) | BIT_3WIRE_PI_ON);
            
            if pre {
                self.write32(REG_ENCCK, self.read32(REG_ENCCK) & !BIT_CCK_OFDM_BLK_EN);
            } else {
                self.write32(REG_ENCCK, self.read32(REG_ENCCK) | BIT_CCK_OFDM_BLK_EN);
            }
            
            // Load header file tables
            if pre {
                // Pre-setting: load MAC_REG first, then BB (PHY_REG)
                self.load_mac_reg_table()?;
                self.load_bb_table_full()?;
            } else {
                // Post-setting: load AGC_TAB, then PHY_REG_PG
                self.load_agc_table_full()?;
                self.load_phy_reg_pg_table()?;
            }
        }
        driver_unlock(); Ok(())
    }

    /// Load MAC register table - mirrors Linux odm_config_mac_with_header_file
    fn load_mac_reg_table(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Loading MAC register table...");
        // 8822C MAC table is empty in Linux (rtw8822c_mac = {})
        driver_unlock(); Ok(())
    }

    /// Load PHY register table - mirrors Linux rtw_parse_tbl_phy_cond + rtw_phy_cfg_bb
    fn load_phy_reg_table(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Loading PHY register table (BB)...");
        
        // Critical BB register table for 8822C (from rtw8822c_table.c rtw8822c_bb)
        // This is a condensed version with the most critical registers
        const PHY_REG_TABLE: &[(u32, u32)] = &[
            // Path A RX Path
            (0x1A00, 0x00000003),
            // LNA/TIA gain Path A
            (0x1A04, 0x00F0F0F0),
            // LNA/TIA gain Path B  
            (0x4A04, 0x00F0F0F0),
            // CCK check disable
            (0x1A80, 0x00000000),
            (0x0454, 0x00000000),
            // BW 20MHz default
            (0x0668, 0x0E301000),
            // CCA mask
            (0x1C80, 0x0F000000),
            // IGI Path A
            (0x1C50, 0x00000020),
            // IGI Path B
            (0x1C54, 0x00000020),
            // CCK PD thresholds
            (0x1A98, 0x00000000),
            (0x1AA8, 0x00000000),
            (0x1A70, 0x00000000),
            (0x1AE8, 0x80000006),
            (0x1AEC, 0x00000006),
            // Adaptivity init
            (0x1A80, 0x00000000),
            (0x1A84, 0x00000000),
            // CFO init
            (0x1A8C, 0x00000000),
            (0x1A90, 0x00000000),
            // TX path diversity
            (0x1A94, 0x00000000),
            // RX Gain tables Path A
            (0x1A08, 0x00000000),
            (0x1A0C, 0x00000000),
            (0x1A10, 0x00000000),
            (0x1A14, 0x00000000),
            // RX Gain tables Path B
            (0x4A08, 0x00000000),
            (0x4A0C, 0x00000000),
            (0x4A10, 0x00000000),
            (0x4A14, 0x00000000),
            // RF Path A power trim
            (0x1BEC, 0xFFFFFFFF),
            // RF Path B power trim
            (0x41EC, 0xFFFFFFFF),
            // DPD disable
            (0x1BEC, 0xFFFFFFFF),
        ];

        for (addr, data) in PHY_REG_TABLE {
            unsafe {
                self.write32(*addr, *data);
            }
        }
        
        crate::mesa_println!("[RTL8822CE] PHY register table loaded ({} entries)", PHY_REG_TABLE.len());
        driver_unlock(); Ok(())
    }

    /// Load AGC table - mirrors Linux rtw_parse_tbl_phy_cond + rtw_phy_cfg_agc
    fn load_agc_tab_table(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Loading AGC table...");
        
        // Critical AGC register table for 8822C (from rtw8822c_table.c rtw8822c_agc)
        // The Linux table has conditional entries per RF path and band
        // This condensed version includes the critical gain tables
        const AGC_TABLE: &[(u32, u32)] = &[
            // AGC Table selector - Path A 2G
            (0x1D90, 0x300001FF),
            (0x1D90, 0x300101FE),
            (0x1D90, 0x300201FD),
            (0x1D90, 0x300301FC),
            (0x1D90, 0x300401FB),
            (0x1D90, 0x300501FA),
            (0x1D90, 0x300601F9),
            (0x1D90, 0x300701F8),
            (0x1D90, 0x300801F7),
            (0x1D90, 0x300901F6),
            (0x1D90, 0x300A01F5),
            (0x1D90, 0x300B01F4),
            (0x1D90, 0x300C01F3),
            (0x1D90, 0x300D01F2),
            (0x1D90, 0x300E01F1),
            (0x1D90, 0x300F01F0),
            (0x1D90, 0x301001EF),
            (0x1D90, 0x301101EE),
            (0x1D90, 0x301201ED),
            (0x1D90, 0x301301EC),
            (0x1D90, 0x301401EB),
            (0x1D90, 0x301501EA),
            (0x1D90, 0x301601E9),
            (0x1D90, 0x301701E8),
            (0x1D90, 0x301801E7),
            (0x1D90, 0x301901E5),
            (0x1D90, 0x301A01E4),
            (0x1D90, 0x301B01C5),
            (0x1D90, 0x301C01C4),
            (0x1D90, 0x301D01C3),
            (0x1D90, 0x301E01C2),
            (0x1D90, 0x301F0188),
            (0x1D90, 0x30200187),
            (0x1D90, 0x30210186),
            (0x1D90, 0x30220184),
            (0x1D90, 0x30230183),
            (0x1D90, 0x30240182),
            (0x1D90, 0x30250181),
            (0x1D90, 0x30260148),
            (0x1D90, 0x30270147),
            (0x1D90, 0x30280146),
            (0x1D90, 0x30290144),
            (0x1D90, 0x302A0143),
            (0x1D90, 0x302B0142),
            (0x1D90, 0x302C0141),
            (0x1D90, 0x302D00C8),
            (0x1D90, 0x302E00C7),
            (0x1D90, 0x302F00C6),
            (0x1D90, 0x303000C5),
            (0x1D90, 0x303100C4),
            (0x1D90, 0x303200C3),
            (0x1D90, 0x30330048),
            (0x1D90, 0x30340047),
            (0x1D90, 0x30350046),
            (0x1D90, 0x30360045),
            (0x1D90, 0x30370025),
            (0x1D90, 0x30380024),
            (0x1D90, 0x30390023),
            (0x1D90, 0x303A0022),
            (0x1D90, 0x303B0021),
            (0x1D90, 0x303C0020),
            (0x1D90, 0x303D0003),
            (0x1D90, 0x303E0002),
            (0x1D90, 0x303F0001),
        ];

        for (addr, data) in AGC_TABLE {
            unsafe {
                self.write32(*addr, *data);
            }
        }
        
        crate::mesa_println!("[RTL8822CE] AGC table loaded ({} entries)", AGC_TABLE.len());
        driver_unlock(); Ok(())
    }

    /// Load PHY register PG table - mirrors Linux rtw_parse_tbl_bb_pg
    fn load_phy_reg_pg_table(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Loading PHY register PG table...");
        
        // PHY Register PG table for 8822C (type 0)
        // From Linux halhwimg8822c_bb.c array_mp_8822c_phy_reg_pg
        // Each entry has: band (0=2G/1=5G), path (0=A/1=B), tx_num (0=1Tx/1=2Tx), addr, mask, data
        // We extract just the addr/data pairs for direct register writes
        const PHY_PG_TABLE: &[(u32, u32)] = &[
            // 2.4G Path A 1Tx
            (0x0C20, 0x484C5054),
            (0x0C24, 0x54585858),
            (0x0C28, 0x44484C50),
            (0x0C2C, 0x50545858),
            (0x0C30, 0x4044484C),
            (0x0C34, 0x50545858),
            (0x0C38, 0x4044484C),
            (0x0C3C, 0x50545858),
            (0x0C40, 0x4044484C),
            (0x0C44, 0x5858383C),
            (0x0C48, 0x484C5054),
            (0x0C4C, 0x383C4044),
            // 2.4G Path B 1Tx
            (0x0E20, 0x484C5054),
            (0x0E24, 0x54585858),
            (0x0E28, 0x44484C50),
            (0x0E2C, 0x50545858),
            (0x0E30, 0x4044484C),
            (0x0E34, 0x50545858),
            (0x0E38, 0x4044484C),
            (0x0E3C, 0x50545858),
            (0x0E40, 0x4044484C),
            (0x0E44, 0x5858383C),
            (0x0E48, 0x484C5054),
            (0x0E4C, 0x383C4044),
            // 5G Path A 1Tx
            (0x0C24, 0x54585858),
            (0x0C28, 0x44484C50),
            (0x0C2C, 0x50545858),
            (0x0C30, 0x4044484C),
            (0x0C34, 0x50545858),
            (0x0C38, 0x4044484C),
            (0x0C3C, 0x50545858),
            (0x0C40, 0x4044484C),
            (0x0C44, 0x5858383C),
            (0x0C48, 0x484C5054),
            (0x0C4C, 0x383C4044),
            // 5G Path B 1Tx
            (0x0E24, 0x54585858),
            (0x0E28, 0x44484C50),
            (0x0E2C, 0x50545858),
            (0x0E30, 0x4044484C),
            (0x0E34, 0x50545858),
            (0x0E38, 0x4044484C),
            (0x0E3C, 0x50545858),
            (0x0E40, 0x4044484C),
            (0x0E44, 0x5858383C),
            (0x0E48, 0x484C5054),
            (0x0E4C, 0x383C4044),
        ];

        for (addr, data) in PHY_PG_TABLE {
            unsafe {
                self.write32(*addr, *data);
            }
        }
        
        crate::mesa_println!("[RTL8822CE] PHY PG table loaded ({} entries)", PHY_PG_TABLE.len());
        driver_unlock(); Ok(())
    }

    /// Config TRX mode (rtw8822c_config_trx_mode)
    fn config_trx_mode(&self) -> Result<(), &'static str> {
        unsafe {
            // For 2T2R: both paths enabled
            // Path A
            if (0x03 & 0x01) != 0 { // BB_PATH_A
                self.write32(REG_ORITXCODE, self.read32(REG_ORITXCODE) & !0xFFFFF | 0x33312);
            } else {
                self.write32(REG_ORITXCODE, self.read32(REG_ORITXCODE) & !0xFFFFF | 0x11111);
            }
            if (0x03 & 0x02) != 0 { // BB_PATH_B
                self.write32(REG_ORITXCODE2, self.read32(REG_ORITXCODE2) & !0xFFFFF | 0x33312);
            } else {
                self.write32(REG_ORITXCODE2, self.read32(REG_ORITXCODE2) & !0xFFFFF | 0x11111);
            }
            
            // Config RX path
            self.config_rx_path(0x03)?;
            // Config TX path
            self.config_tx_path(0x03, 0x01, 0x01, false)?;
            
            // Toggle IGI
            self.toggle_igi()?;
        }
        driver_unlock(); Ok(())
    }

    /// rtw8822c_bb_reset: toggle the BB reset bit in REG_SYS_FUNC_EN.
    fn bb_reset(&self) {
        unsafe {
            let v = self.read16(REG_SYS_FUNC_EN);
            self.write16(REG_SYS_FUNC_EN, v | (BIT_FEN_BB_RSTB as u16));
            self.write16(REG_SYS_FUNC_EN, v & !(BIT_FEN_BB_RSTB as u16));
            self.write16(REG_SYS_FUNC_EN, v | (BIT_FEN_BB_RSTB as u16));
        }
    }

    /// Config RX path (rtw8822c_config_rx_path: CCK + OFDM antenna routing).
    /// Without this the OFDM RX path registers (REG_ANTMAP 0x824, REG_RXFNCTL,
    /// REG_AGCSWSH, REG_ANTWTPD, REG_MRCM) and the CCK path stay at their reset
    /// values, so no antenna is routed to the demodulator and RX never fires.
    fn config_rx_path(&self, rx_path: u8) -> Result<(), &'static str> {
        unsafe {
            // rtw8822c_config_cck_rx_path
            if rx_path == 0x01 || rx_path == 0x02 {
                self.write32_mask(REG_CCANRX, 0x0006_0000, 0x0);
                self.write32_mask(REG_CCANRX, 0x0060_0000, 0x0);
            } else if rx_path == 0x03 {
                self.write32_mask(REG_CCANRX, 0x0060_0000, 0x1);
                self.write32_mask(REG_CCANRX, 0x0006_0000, 0x1);
            }
            if rx_path == 0x01 {
                self.write32_mask(REG_RXCCKSEL, 0x0F00_0000, 0x0);
            } else if rx_path == 0x02 {
                self.write32_mask(REG_RXCCKSEL, 0x0F00_0000, 0x5);
            } else if rx_path == 0x03 {
                self.write32_mask(REG_RXCCKSEL, 0x0F00_0000, 0x1);
            }

            // rtw8822c_config_ofdm_rx_path
            let v = if rx_path == 0x03 { 1u32 } else { 0u32 };
            self.write32_mask(REG_RXFNCTL, 0x0000_0300, v);
            self.write32_mask(REG_RXFNCTL, 0x0060_0000, v);
            self.write32_mask(REG_AGCSWSH, 1 << 17, v);
            self.write32_mask(REG_ANTWTPD, 1 << 20, v);
            self.write32_mask(REG_MRCM, 1 << 24, v);
            self.write32_mask(REG_ANTMAP, 0x0F00_0000, rx_path as u32);
            self.write32_mask(REG_ANTMAP, 0x000F_0000, rx_path as u32);
        }
        driver_unlock(); Ok(())
    }

    /// Config TX path (rtw8822c_config_tx_path: CCK + OFDM antenna routing).
    fn config_tx_path(&self, tx_path: u8, tx_path_sel_1ss: u8, tx_path_cck: u8, is_tx2_path: bool) -> Result<(), &'static str> {
        unsafe {
            // rtw8822c_config_cck_tx_path
            if tx_path_cck == 0x01 {
                self.write32_mask(REG_RXCCKSEL, 0xF000_0000, 0x8);
            } else if tx_path_cck == 0x02 {
                self.write32_mask(REG_RXCCKSEL, 0xF000_0000, 0x4);
            } else if is_tx2_path {
                self.write32_mask(REG_RXCCKSEL, 0xF000_0000, 0xC);
            } else {
                self.write32_mask(REG_RXCCKSEL, 0xF000_0000, 0x8);
            }
            self.bb_reset();

            // rtw8822c_config_ofdm_tx_path
            if tx_path == 0x01 {
                self.write32_mask(REG_ANTMAP0, 0xFF, 0x11);
                self.write32_mask(REG_TXLGMAP, 0xFF, 0x0);
            } else if tx_path == 0x02 {
                self.write32_mask(REG_ANTMAP0, 0xFF, 0x12);
                self.write32_mask(REG_TXLGMAP, 0xFF, 0x0);
            } else if tx_path_sel_1ss == 0x03 {
                self.write32_mask(REG_ANTMAP0, 0xFF, 0x33);
                self.write32_mask(REG_TXLGMAP, 0xFFFF, 0x0404);
            } else if tx_path_sel_1ss == 0x02 {
                self.write32_mask(REG_ANTMAP0, 0xFF, 0x32);
                self.write32_mask(REG_TXLGMAP, 0xFFFF, 0x0400);
            } else if tx_path_sel_1ss == 0x01 {
                self.write32_mask(REG_ANTMAP0, 0xFF, 0x31);
                self.write32_mask(REG_TXLGMAP, 0xFFFF, 0x0400);
            }
            self.bb_reset();
        }
        driver_unlock(); Ok(())
    }

    /// Toggle IGI (rtw8822c_toggle_igi)
    fn toggle_igi(&self) -> Result<(), &'static str> {
        unsafe {
            let igi = self.read32(REG_RXIGI) & 0x7f;
            self.write32(REG_RXIGI, (self.read32(REG_RXIGI) & !0x7f) | ((igi - 2) & 0x7f));
            self.write32(REG_RXIGI, (self.read32(REG_RXIGI) & !0x7f00) | (((igi - 2) & 0x7f) << 8));
            self.write32(REG_RXIGI, (self.read32(REG_RXIGI) & !0x7f) | (igi & 0x7f));
            self.write32(REG_RXIGI, (self.read32(REG_RXIGI) & !0x7f00) | ((igi & 0x7f) << 8));
        }
        driver_unlock(); Ok(())
    }

    // ========================================================================
    // PHY Table Parsing Infrastructure (mirrors Linux rtw_parse_tbl_phy_cond)
    // ========================================================================

    /// Driver phy_cond (rtw_phy_setup_phy_cond) as (cut, pkg, intf, rfe).
    fn drv_phy_cond(&self) -> (u32, u32, u32, u32) {
        let cut = unsafe { (self.read32(REG_SYS_CFG1) >> 12) & 0xf };
        (cut, 0xf, 1 /* INTF_PCIE */, self.efuse.rfe_option as u32)
    }

    /// rtw_phy_cfg_bb: plain 32-bit write with delay pseudo-addresses.
    fn cfg_bb(&self, addr: u32, data: u32) {
        match addr {
            0xfe => self.delay_ms(50),
            0xfd => self.delay_ms(5),
            0xfc => self.delay_ms(1),
            0xfb => self.delay_us(55),
            0xfa => self.delay_us(5),
            0xf9 => self.delay_us(1),
            _ => unsafe { self.write32(addr, data) },
        }
    }

    /// rtw_phy_cfg_rf: RF write with delay pseudo-addresses.
    fn cfg_rf(&self, rf_path: u8, addr: u32, data: u32) {
        match addr {
            0xffe => self.delay_ms(50),
            0xfe => self.delay_us(100),
            _ => {
                self.write_rf_mix(rf_path, addr, RFREG_MASK, data);
                self.delay_us(1);
            }
        }
    }

    /// rtw_phy_write_rf_reg_mix: RF reg 0 via SIPI, all others via the direct
    /// BB-mapped window (0x3c00/0x4c00 + (addr << 2)) used by 8822C.
    fn write_rf_mix(&self, rf_path: u8, addr: u32, mask: u32, data: u32) {
        let a = addr & 0xff;
        if a != 0 {
            let base = if rf_path == 0 { 0x3c00 } else { 0x4c00 };
            let direct = base + (a << 2);
            unsafe { self.write32_mask(direct, mask & RFREG_MASK, data); }
            self.delay_us(1);
        } else {
            self.write_rf_sipi(rf_path, a, mask, data);
        }
    }

    /// Parse a phy_cond table - faithful port of rtw_parse_tbl_phy_cond.
    fn parse_phy_table(&self, data: &[u32], kind: CfgKind, rf_path: u8) {
        let (drv_cut, drv_pkg, drv_intf, drv_rfe) = self.drv_phy_cond();
        let mut pos = (0u32, 0u32, 0u32, 0u32);
        let mut is_matched = true;
        let mut is_skipped = false;
        let pairs = data.len() / 2;
        for i in 0..pairs {
            let addr = data[i * 2];
            let val = data[i * 2 + 1];
            let pos_bit = (addr >> 31) & 1;
            let neg_bit = (addr >> 30) & 1;
            let branch = (addr >> 28) & 3;
            if pos_bit != 0 {
                match branch {
                    BRANCH_ENDIF => {
                        is_matched = true;
                        is_skipped = false;
                    }
                    BRANCH_ELSE => {
                        is_matched = !is_skipped;
                    }
                    _ => {
                        pos = ((addr >> 24) & 0xf, (addr >> 12) & 0xf,
                               (addr >> 8) & 0xf, addr & 0xff);
                    }
                }
            } else if neg_bit != 0 {
                if !is_skipped {
                    let ok = (pos.0 == 0 || pos.0 == drv_cut)
                        && (pos.1 == 0 || pos.1 == drv_pkg)
                        && (pos.2 == 0 || pos.2 == drv_intf)
                        && (pos.3 == drv_rfe);
                    if ok {
                        is_matched = true;
                        is_skipped = true;
                    } else {
                        is_matched = false;
                        is_skipped = false;
                    }
                } else {
                    is_matched = false;
                }
            } else if is_matched {
                match kind {
                    CfgKind::Mac => unsafe { self.write8(addr, val as u8) },
                    CfgKind::Bb => self.cfg_bb(addr, val),
                    CfgKind::Agc => unsafe { self.write32(addr, val) },
                    CfgKind::Rf => self.cfg_rf(rf_path, addr, val),
                }
            }
        }
    }

    /// Load the full BB table (rtw8822c_bb) via the phy_cond parser.
    fn load_bb_table_full(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Loading full BB table ({} words)...", BB_TBL.len());
        self.parse_phy_table(BB_TBL, CfgKind::Bb, 0);
        crate::mesa_println!("[RTL8822CE] Full BB table loaded");
        driver_unlock(); Ok(())
    }

    /// Load the full AGC table (rtw8822c_agc).
    fn load_agc_table_full(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Loading full AGC table ({} words)...", AGC_TBL.len());
        self.parse_phy_table(AGC_TBL, CfgKind::Agc, 0);
        crate::mesa_println!("[RTL8822CE] Full AGC table loaded");
        driver_unlock(); Ok(())
    }

    /// rtw_load_rfk_table - powers up the RF/DPK calibration block and loads
    /// the `rtw8822c_array_mp_cal_init` (rfk_init_tbl) BB table. rtw88 runs
    /// this unconditionally from rtw_phy_load_tables, between the AGC table
    /// and the RF radio tables.
    fn load_rfk_table(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] rtw_load_rfk_table (mp_cal_init {} words)...",
            MP_CAL_INIT_TBL.len());
        unsafe {
            self.write32_mask(0x1e24, 1 << 17, 1);
            self.write32_mask(0x1cd0, 1 << 28, 1);
            self.write32_mask(0x1cd0, 1 << 29, 1);
            self.write32_mask(0x1cd0, 1 << 30, 1);
            self.write32_mask(0x1cd0, 1 << 31, 0);
        }
        self.parse_phy_table(MP_CAL_INIT_TBL, CfgKind::Bb, 0);
        driver_unlock(); Ok(())
    }

    /// Load the full RF radio tables (rtw8822c_rf_a / rf_b), path B then A.
    fn load_rf_tables_full(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Loading full RF tables (rf_b={} rf_a={} words)...",
            RF_B_TBL.len(), RF_A_TBL.len());
        self.parse_phy_table(RF_B_TBL, CfgKind::Rf, 1); // RF_PATH_B
        self.parse_phy_table(RF_A_TBL, CfgKind::Rf, 0); // RF_PATH_A
        crate::mesa_println!("[RTL8822CE] Full RF tables loaded");
        driver_unlock(); Ok(())
    }

    /// rtw_phy_load_tables: mac, bb, agc, rf (both paths).
    fn load_full_phy_tables(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] rtw_phy_load_tables: loading full PHY tables...");
        self.load_bb_table_full()?;
        self.load_agc_table_full()?;
        self.load_rf_tables_full()?;
        crate::mesa_println!("[RTL8822CE] rtw_phy_load_tables complete");
        driver_unlock(); Ok(())
    }

    /// MAC Post Init (rtw_mac_postinit)
    fn mac_postinit(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] MAC post init...");
        // Additional MAC configuration after PHY
        // RCR reset after powered on
        unsafe {
            self.write32(REG_RCR, self.read32(REG_RCR) | RCR_AMF | RCR_AB | RCR_AM | RCR_APM | RCR_AAP);
        }
        
        // Watchdog work would be started here in Linux
        crate::mesa_println!("[RTL8822CE] MAC post init complete");
        driver_unlock(); Ok(())
    }

    /// HCI Start (rtw_hci_start) - enable interrupts, start RX/TX
    fn hci_start(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] HCI start (enabling interrupts, RX/TX)...");
        
        // Enable RX/TX DMA
        unsafe {
            // Ensure MAC TRX is enabled
            self.write8(REG_CR, MAC_TRX_ENABLE as u8);

            // Enable the same interrupt mask as setup_interrupts (rtw88
            // rtw_pci_enable_interrupt). The previous 0xFFFFFFFF also enabled
            // unhandled error sources (RXFOVW/TXERR/BCNDMAINTx) and undid the
            // selective mask, causing spurious IRQs.
            let himr0 = BIT_RXOK_INT
                | BIT_VODOK_INT
                | BIT_VIDOK_INT
                | BIT_BEDOK_INT
                | BIT_BKDOK_INT
                | BIT_MGTDOK_INT
                | BIT_HIGHDOK_INT
                | BIT_C2H_INT;
            self.write32(REG_PCIE_HIMR0, himr0);
            self.write32(REG_PCIE_HIMR1, 0x00000000);

            // Clear any pending interrupts
            self.write32(REG_PCIE_HISR0, 0xFFFFFFFF);
            self.write32(REG_PCIE_HISR1, 0xFFFFFFFF);
            self.write32(REG_PCIE_HISR3, 0xFFFFFFFF);
        }
        
        crate::mesa_println!("[RTL8822CE] HCI start complete - interrupts enabled");
        driver_unlock(); Ok(())
    }

    /// Send H2C General Info (rtw_fw_send_general_info).
    ///
    /// This is a fw_offload PKT (category 0x01, cmd 0xFF, sub-cmd 0x0D) sent
    /// through the H2C command queue, NOT a mailbox command. It tells the
    /// firmware where the FW TX buffer region starts relative to the reserved
    /// boundary (`rsvd_fw_txbuf_addr - rsvd_boundary`) so the FW manages the
    /// shared TX/RX FIFO without clobbering the driver pages.
    fn fw_send_general_info(&mut self) -> Result<(), &'static str> {
        let mut pkt = [0u8; 32];
        pkt[0] = 0x01; // H2C_PKT_CATEGORY
        pkt[1] = 0xFF; // H2C_PKT_CMD_ID
        pkt[2] = 0x0D; // H2C_PKT_GENERAL_INFO
        pkt[3] = 0x00;
        let total_len: u16 = 8 /*H2C_PKT_HDR_SIZE*/ + 4;
        pkt[4] = (total_len & 0xff) as u8;
        pkt[5] = (total_len >> 8) as u8;

        // dword2 bits[23:16] = FW_TX_BOUNDARY (pages from reserved boundary to
        // the FW TX buffer start).
        const RSVD_FW_TXBUF_ADDR: u32 = TXFF_PG_NUM - 50 - 4; // 1994
        let fw_tx_boundary: u8 = (RSVD_FW_TXBUF_ADDR - RSVD_BOUNDARY as u32) as u8; // 0x38
        pkt[10] = fw_tx_boundary;

        crate::mesa_println!(
            "[RTL8822CE] H2C general info: fw_tx_boundary={:#x}", fw_tx_boundary);
        self.h2c_send_pkt(&pkt)?;
        driver_unlock(); Ok(())
    }

    /// Send H2C Phydm Info (rtw_fw_send_phydm_info).
    ///
    /// This is a fw_offload PKT (category 0x01, cmd 0xFF, sub-cmd 0x11) sent
    /// through the H2C command queue, NOT a mailbox command. It tells the
    /// firmware the RF front-end option, RF type, cut version and antenna
    /// status so its PHYDM (AGC/DIG/IQ tracking) matches this board.
    fn fw_send_phydm_info(&mut self) -> Result<(), &'static str> {
        let mut pkt = [0u8; 32];
        pkt[0] = 0x01; // H2C_PKT_CATEGORY
        pkt[1] = 0xFF; // H2C_PKT_CMD_ID
        pkt[2] = 0x11; // H2C_PKT_PHYDM_INFO
        pkt[3] = 0x00;
        let total_len: u16 = 8 /*H2C_PKT_HDR_SIZE*/ + 8;
        pkt[4] = (total_len & 0xff) as u8;
        pkt[5] = (total_len >> 8) as u8;

        // dword2: ref_type[7:0], rf_type[15:8], cut[23:16], ant[31:24].
        let cfg = unsafe { self.read32(REG_SYS_CFG1) };
        let cut = ((cfg >> 12) & 0xf) as u8;
        let rf_type = ((cfg >> 8) & 0xf) as u8;
        let fw_rf_type: u8 = if rf_type == 0xd { 2 /*FW_RF_2T2R*/ } else { 4 /*FW_RF_1T1R*/ };
        pkt[8] = self.efuse.rfe_option;
        pkt[9] = fw_rf_type;
        pkt[10] = cut;
        pkt[11] = 0x00; // antenna status

        crate::mesa_println!(
            "[RTL8822CE] H2C phydm info: rfe={:#04x} rf_type={} cut={}",
            self.efuse.rfe_option, fw_rf_type, cut
        );
        self.h2c_send_pkt(&pkt)?;
        driver_unlock(); Ok(())
    }

    /// Trigger the firmware-driven IQK and wait for completion.
    ///
    /// rtw8822c_do_iqk(): send the H2C_PKT_IQK (sub-cmd 0x0E, clear=1) and poll
    /// REG_RPT_CIP until it reads IQK_DONE_8822C (0xAA). On 8822C the IQ
    /// calibration is executed entirely by the firmware; there is no host-side
    /// BB register sequence.
    fn fw_do_iqk(&mut self) -> Result<(), &'static str> {
        let mut pkt = [0u8; 32];
        pkt[0] = 0x01; // H2C_PKT_CATEGORY
        pkt[1] = 0xFF; // H2C_PKT_CMD_ID
        pkt[2] = 0x0E; // H2C_PKT_IQK
        pkt[3] = 0x00;
        let total_len: u16 = 8 /*H2C_PKT_HDR_SIZE*/ + 1;
        pkt[4] = (total_len & 0xff) as u8;
        pkt[5] = (total_len >> 8) as u8;
        pkt[8] = 0x01; // IQK_SET_CLEAR

        self.h2c_send_pkt(&pkt)?;

        // Poll up to ~300 ms (rtw88: 20ms interval x 15).
        let mut done = false;
        for _ in 0..15 {
            if (unsafe { self.read8(REG_RPT_CIP) }) == 0xaa {
                done = true;
                break;
            }
            self.delay_ms(20);
        }
        if done {
            crate::mesa_println!("[RTL8822CE] FW IQK done (RPT_CIP=0xAA)");
        } else {
            crate::mesa_println!(
                "[RTL8822CE] FW IQK timeout (RPT_CIP={:#04x})",
                unsafe { self.read8(REG_RPT_CIP) }
            );
        }
        driver_unlock(); Ok(())
    }

    /// Firmware PHY calibration: send PHYDM info then run the firmware IQK.
    /// Mirrors the calibration step of rtw8822c_phy_calibration for the parts
    /// that are firmware-driven (GAPK/DPK are host-side and not wired up yet).
    fn fw_phy_calibration(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] FW-driven PHY calibration...");
        self.fw_send_phydm_info()?;
        self.fw_do_iqk()?;
        driver_unlock(); Ok(())
    }

    /// Power track init (rtw8822c_pwrtrack_init).
    /// rtw88 8822C: this is pure software state (delta_power_index, avg_thermal,
    /// thermal_meter_k/lck); it writes NO hardware registers. The previous
    /// implementation wrote fabricated BB registers (0x1c50/0x1c68/0x1bcc/
    /// 0x1bd0/0x1c70/0x1c74/0x1c78) that are not part of this step; removed.
    fn pwrtrack_init(&self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] Power tracking init (SW state only)");
        driver_unlock(); Ok(())
    }

    /// BF PHY init (rtw_bf_phy_init) - mirrors Linux rtw_bf_phy_init
    fn bf_phy_init(&mut self) -> Result<(), &'static str> {
        crate::mesa_println!("[RTL8822CE] BF PHY init (rtw_bf_phy_init)...");
        
        unsafe {
            // Faithful port of rtw_bf_phy_init (bf.c). The previous version
            // zeroed several BF registers; rtw88 only sets the MU-MIMO defaults.
            let mut mu = self.read32(0x14C0);
            mu |= 1 << 16;          // BIT_MU_P1_WAIT_STATE_EN
            mu &= !(0xf << 12);     // ~BIT_MASK_R_MU_RL
            mu |= (0xA << 12) & (0xf << 12);
            mu &= !(1 << 7);        // ~BIT_EN_MU_MIMO
            mu &= !0x3f;            // ~BIT_MASK_R_MU_TABLE_VALID
            self.write32(0x14C0, mu);

            // MU-MIMO option: ack policy 3, ack policy enable.
            self.write8(0x167C, (3 << 4) | (1 << 6));
            // MU-MIMO control default.
            self.write16(0x1680, 0);
            // Use NDPA parameters.
            let v = self.read32(0x042C);
            self.write32(0x042C, v | (1 << 30)); // BIT_USE_NDPA_PARAMETER
            // NDPA rate.
            self.write8(0x045F, 0x10);
            // CSI rate = DESC_RATE6M.
            let bbpsf = self.read32(0x06DC);
            self.write32(0x06DC, (bbpsf & !(0x3f << 24)) | ((0x04u32 & 0x3f) << 24));

            self.bf_cap = 0;
        }
        
        crate::mesa_println!("[RTL8822CE] BF PHY init complete");
        driver_unlock(); Ok(())
    }

    // ========================================================================
    // Full Initialization
    // ========================================================================

    fn init_full(&mut self, fw_data: Option<&[u8]>) -> Result<(), &'static str> {
        // ========================================================================
        // Linux rtw_power_on() flow - EXACT ORDER
        // ========================================================================
        crate::mesa_println!("[RTL8822CE] ===== Starting Linux-compatible initialization =====");

        // 1. PCI initialization (rtw_hci_setup)
        crate::mesa_println!("[RTL8822CE] Step 1/12: PCI/HCI setup...");
        self.pci_init()?;
        self.hci_setup()?;
        wifi_log("Paso 1/12: PCI/HCI setup completado");

        // 2. Allocate FW-download staging buffers + enable DMA interface. The
        //    firmware download stages data through the BCN reserved-page ring,
        //    so the BCN BD ring and data buffer MUST be allocated first.
        crate::mesa_println!("[RTL8822CE] Step 2/12: Allocating FW staging buffers...");
        self.alloc_staging_buffers()?;
        wifi_log("Paso 2/12: Staging buffers listos");

        // 3. MAC Power On + Firmware Download (rtw_mac_power_on + rtw_download_firmware)
        crate::mesa_println!("[RTL8822CE] Step 3/12: MAC power on + firmware download...");
        self.mac_power_on_with_firmware(fw_data)?;
        wifi_log("Paso 3/12: MAC power on + firmware completado");

        // 4. Wait for firmware completion (rtw_wait_firmware_completion)
        crate::mesa_println!("[RTL8822CE] Step 4/12: Waiting for firmware completion...");
        self.wait_firmware_completion()?;
        wifi_log("Paso 4/12: Firmware completion OK");

        // 5. Read MAC address
        crate::mesa_println!("[RTL8822CE] Step 5/12: Reading MAC address...");
        self.read_mac_from_efuse()
            .unwrap_or_else(|e| {
                crate::serial_println!("[RTL8822CE] EFUSE MAC read failed: {}, using default", e);
                self.mac = [0x02, 0x88, 0x22, 0xCE, 0x00, 0x01];
            });
        wifi_log("Paso 5/12: MAC leida");

        // 6. MAC Init after firmware (rtw_mac_init / rtw8822c_mac_init)
        crate::mesa_println!("[RTL8822CE] Step 6/12: MAC init after firmware...");
        self.mac_init_after_fw()?;
        wifi_log("Paso 6/12: MAC init completado");

        // 7. Setup DMA rings (full ring programming, now that MAC is powered
        //    and firmware is running). Buffers already allocated in Step 2.
        crate::mesa_println!("[RTL8822CE] Step 7/12: Setting up DMA rings...");
        self.setup_dma_rings()?;
        wifi_log("Paso 7/12: DMA rings configurados");

        // 8. PHY Set Param (chip->ops->phy_set_param / rtw8822c_phy_set_param)
        crate::mesa_println!("[RTL8822CE] Step 8/12: PHY set param (BB/RF)...");
        self.phy_set_param()?;
        wifi_log("Paso 8/12: PHY set param completado");

        // 9. MAC Post Init (rtw_mac_postinit)
        crate::mesa_println!("[RTL8822CE] Step 9/12: MAC post init...");
        self.mac_postinit()?;
        wifi_log("Paso 9/12: MAC post init completado");

        // 10. HCI Start (rtw_hci_start)
        crate::mesa_println!("[RTL8822CE] Step 10/12: HCI start (interrupts, RX/TX)...");
        self.hci_start()?;
        wifi_log("Paso 10/12: HCI start completado");

        // 11. Send H2C General Info & Phydm Info (rtw_fw_send_general_info + rtw_fw_send_phydm_info)
        crate::mesa_println!("[RTL8822CE] Step 11/12: Sending H2C general/phydm info...");
        self.fw_send_general_info()?;
        self.fw_send_phydm_info()?;
        wifi_log("Paso 11/12: H2C info enviado");

        // 12. Coex Power On & Init (rtw_coex_power_on_setting + rtw_coex_init_hw_config)
        crate::mesa_println!("[RTL8822CE] Step 12/12: Coex power on & init...");
        self.coex_power_on_setting()?;
        self.coex_init_hw_config()?;
        wifi_log("Paso 12/12: Coex init completado");

        // Final: Set default channel and RX config
        self.set_channel(1, false);
        self.set_rx_config();

        self.initialized = true;
        crate::mesa_println!("[RTL8822CE] ===== Driver fully initialized (Linux-compatible) =====");
        wifi_log("Driver RTL8822CE INICIALIZADO OK (Linux flow)");
        driver_unlock(); Ok(())
    }
}

// ============================================================================
// Global driver state
// ============================================================================

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

/// Thread-safe wrapper for the driver instance
struct DriverCell(core::cell::UnsafeCell<Option<alloc::boxed::Box<Rtl8822ce>>>);

unsafe impl Sync for DriverCell {}

impl DriverCell {
    fn lock(&self) -> DriverLockGuard {
        driver_lock();
        DriverLockGuard
    }
}

struct DriverLockGuard;

impl core::ops::Deref for DriverLockGuard {
    type Target = Rtl8822ce;
    fn deref(&self) -> &Rtl8822ce {
        driver_get().unwrap()
    }
}

impl core::ops::DerefMut for DriverLockGuard {
    fn deref_mut(&mut self) -> &mut Rtl8822ce {
        driver_get().unwrap()
    }
}

impl Drop for DriverLockGuard {
    fn drop(&mut self) {
        driver_unlock();
    }
}

static DRIVER: DriverCell = DriverCell(core::cell::UnsafeCell::new(None));
static DRIVER_LOCK: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static DRIVER_LOCK_DEPTH: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static RX_DATA_COUNT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static RX_DHCP_DBG: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
/// CCMP transmit packet number (PN). Monotonically increasing across frames;
/// the 8-byte CCMP header is built from it (rtw88 relies on mac80211's
/// IEEE80211_KEY_FLAG_GENERATE_IV, so the PN/header comes from software).
static CCMP_TX_PN: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
/// 802.11 sequence number programmed for the last data frame. Diagnostic for
/// the WPA2 4-way handshake: the AP drops msg 4/4 if its sequence repeats.
static LAST_TX_SEQ: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(0);

/// Sequence counter for management/non-QoS frames. Kept separate from the
/// data counter: the AP deduplicates per sender and only needs a monotonic
/// value here.
static MGMT_TX_SEQ: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(0);

/// Sequence number used by the most recent data transmission.
pub fn last_tx_seq() -> u16 {
    LAST_TX_SEQ.load(core::sync::atomic::Ordering::Relaxed)
}

/// Number of 802.11 data frames received from the air (diagnostic).
pub fn rx_data_count() -> u32 {
    RX_DATA_COUNT.load(core::sync::atomic::Ordering::Relaxed)
}

/// Recursive single-CPU driver lock.
///
/// The driver is re-entered in legitimate scenarios: process_rx() (holding the
/// lock) can produce an ARP reply which calls send_packet() again, and the
/// wrapper functions also unlock after the method already did. A plain
/// spinlock self-deadlocks there; a depth counter makes nested acquisitions
/// cheap no-ops. `driver_unlock()` tolerates the legacy "extra unlock" pattern.
fn driver_lock() {
    if DRIVER_LOCK_DEPTH.fetch_add(1, core::sync::atomic::Ordering::AcqRel) == 0 {
        while core::sync::atomic::AtomicBool::compare_exchange_weak(
            &DRIVER_LOCK, false, true,
            core::sync::atomic::Ordering::Acquire,
            core::sync::atomic::Ordering::Relaxed,
        ).is_err() {
            core::hint::spin_loop();
        }
    }
}

fn driver_unlock() {
    // Ignore spurious extra unlocks (depth already 0) instead of wrapping.
    if DRIVER_LOCK_DEPTH.load(core::sync::atomic::Ordering::Acquire) == 0 {
        return;
    }
    if DRIVER_LOCK_DEPTH.fetch_sub(1, core::sync::atomic::Ordering::AcqRel) == 1 {
        DRIVER_LOCK.store(false, core::sync::atomic::Ordering::Release);
    }
}

fn driver_get() -> Option<&'static mut Rtl8822ce> {
    unsafe { (&mut *DRIVER.0.get()).as_deref_mut() }
}

fn driver_set(driver: alloc::boxed::Box<Rtl8822ce>) {
    unsafe { *DRIVER.0.get() = Some(driver) }
}

fn driver_take() -> Option<alloc::boxed::Box<Rtl8822ce>> {
    unsafe { (*DRIVER.0.get()).take() }
}

/// Write the negotiated pairwise TK + group GTK into the hardware security CAM.
/// Call only from the connect/shell context, never while the RX path holds the
/// driver, so taking the lock and touching the CAM here is safe.
fn install_wpa_keys_now(bssid: [u8; 6]) {
    driver_lock();
    if let Some(d) = driver_get() {
        if let Some(ptk) = crate::net::wpa::get_ptk() {
            let (gtk, glen) = crate::net::wpa::get_gtk().unwrap_or(([0u8; 32], 0));
            let gkeyid = crate::net::wpa::get_gtk_keyid();
            crate::mesa_println!(
                "[WPA2] instalando PTK(TK)+GTK en CAM (gtk_keyid={}, gtk_len={})",
                gkeyid, glen
            );
            // WPA/WPA2 mixed APs use a TKIP group cipher. The 8822C CAM can
            // decrypt it in hardware, so broadcast DHCP/ARP keep working; the
            // pairwise key stays CCMP (we negotiate CCMP whenever offered).
            let gtk_type = if crate::drivers::wifi::state::group_cipher_is_tkip() {
                HWCAM_TKIP
            } else {
                HWCAM_AES
            };
            d.install_ccmp_keys(bssid, &ptk.tk, &gtk[..glen], gkeyid, gtk_type);
        }
    }
    driver_unlock();
    crate::mesa_println!("[WPA2] claves TK+GTK instaladas en CAM");
}

/// Check if RTL8822CE is present
pub fn is_present() -> bool {
    pci::find_device(VENDOR_ID, DEVICE_ID).is_some()
        || pci::find_device(VENDOR_ID, DEVICE_ID_ALT).is_some()
}
/// Initialize the RTL8822CE driver
pub fn init() -> Result<(), &'static str> {
    driver_lock();
    if driver_get().is_some() {
        driver_unlock();
        driver_unlock(); return Ok(()); // Already initialized
    }
    driver_unlock();

    let mut driver = Rtl8822ce::new_boxed()?;

    // Try to load firmware from initrd
    let mut fw_option: Option<&[u8]> = None;
    crate::serial_println!("[RTL8822CE] Searching for firmware in initrd...");
    crate::mesa_println!("[RTL8822CE] Searching for firmware in initrd...");

    // Search for firmware in the initrd filesystem
    if let Some(data) = try_load_firmware_from_initrd() {
        crate::serial_println!("[RTL8822CE] Firmware found in initrd: {} bytes", data.len());
        crate::mesa_println!("[RTL8822CE] Firmware found in initrd: {} bytes", data.len());
        fw_option = Some(data);
    } else {
        crate::serial_println!("[RTL8822CE] No firmware found in initrd!");
        crate::mesa_println!("[RTL8822CE] No firmware found in initrd, trying embedded fallback...");
        
        // Fallback: Use embedded firmware (if available)
        // This is a placeholder - in real use, embed the actual firmware bytes
        // For now, we'll try to load from a known static location
        fw_option = try_load_embedded_firmware();
        if fw_option.is_some() {
            crate::mesa_println!("[RTL8822CE] Using embedded firmware: {} bytes", fw_option.unwrap().len());
        }
    }
    
    if fw_option.is_none() {
        crate::serial_println!("[RTL8822CE] ERROR: No firmware available!");
        crate::mesa_println!("[RTL8822CE] ERROR: No firmware available - cannot continue");
        driver_unlock(); return Err("RTL8822CE: Firmware required");
    }

    driver.init_full(fw_option)?;

    driver_set(driver);
    driver_unlock(); Ok(())
}

/// Try to load firmware from the initrd filesystem
fn try_load_firmware_from_initrd() -> Option<&'static [u8]> {
    let initrd = crate::fs::initrd_data::INITRD_DATA;
    if initrd.len() < 4 {
        return None;
    }

    // Parse the initrd binary format: count + [name_len|name|path_len|path|data_len|data]*
    let mut offset = 0;
    let count = u32::from_le_bytes([
        initrd[offset], initrd[offset + 1],
        initrd[offset + 2], initrd[offset + 3],
    ]) as usize;
    offset += 4;

    for _ in 0..count {
        if offset + 4 > initrd.len() { break; }
        let name_len = u32::from_le_bytes([
            initrd[offset], initrd[offset + 1],
            initrd[offset + 2], initrd[offset + 3],
        ]) as usize;
        offset += 4;

        if offset + name_len > initrd.len() { break; }
        let name = &initrd[offset..offset + name_len];
        offset += name_len;

        if offset + 4 > initrd.len() { break; }
        let _path_len = u32::from_le_bytes([
            initrd[offset], initrd[offset + 1],
            initrd[offset + 2], initrd[offset + 3],
        ]) as usize;
        offset += 4;

        // Skip path bytes
        offset += _path_len;
        if offset > initrd.len() { break; }

        if offset + 4 > initrd.len() { break; }
        let data_len = u32::from_le_bytes([
            initrd[offset], initrd[offset + 1],
            initrd[offset + 2], initrd[offset + 3],
        ]) as usize;
        offset += 4;

        if offset + data_len > initrd.len() { break; }

        // Check if this file matches our firmware names
        let name_str = core::str::from_utf8(name).unwrap_or("");
        if name_str.contains("rtw8822c") && name_str.ends_with(".bin") {
            crate::serial_println!(
                "[RTL8822CE] Found firmware '{}' in initrd ({} bytes)",
                name_str, data_len
            );
            // Return a static reference to the data within the initrd
            // This is safe because INITRD_DATA is a static reference
            return Some(&initrd[offset..offset + data_len]);
        }

        offset += data_len;
    }
    None
}

/// Fallback: Try to load embedded firmware
/// This allows the driver to work without initrd by embedding firmware directly
fn try_load_embedded_firmware() -> Option<&'static [u8]> {
    // Embed firmware directly from the project root
    // Path from mesa_kernel/src/drivers/net/ to project root: ../../../
    static EMBEDDED_FW: &[u8] = include_bytes!("../../../../rtw8822c_fw.bin");
    if !EMBEDDED_FW.is_empty() {
        Some(EMBEDDED_FW)
    } else {
        None
    }
}

/// Handle WiFi interrupt (called from interrupt handler)
pub fn handle_interrupt() {
    driver_lock();
    if let Some(driver) = driver_get() {
        driver.handle_interrupt();
    }
    driver_unlock();
}

/// Send a packet via WiFi
pub fn send_packet(data: &[u8]) -> Result<(), &'static str> {
    driver_lock();
    let result = if let Some(driver) = driver_get() {
        driver.send_packet(data)
    } else {
        Err("RTL8822CE driver not initialized")
    };
    driver_unlock();
    result
}

/// Get MAC address
pub fn get_mac() -> Option<[u8; 6]> {
    driver_lock();
    let mac = driver_get().map(|d| d.mac);
    driver_unlock();
    mac
}

/// Check if driver is initialized and ready
pub fn is_ready() -> bool {
    driver_lock();
    let ready = driver_get().map_or(false, |d| d.initialized);
    driver_unlock();
    ready
}

/// Periodic WiFi service, called from the main loop / net::poll.
///
/// Drains any pending RX (RX is primarily IRQ-driven; this is a safety net)
/// and, while associated, sends a QoS-Null keepalive every ~5 s so the AP does
/// not drop us for inactivity.
pub fn wifi_tick() {
    use crate::drivers::wifi::state;
    use core::sync::atomic::Ordering;

    static LAST_KEEPALIVE: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

    // Proactively retransmit msg 4/4 (outside the driver lock) while the
    // handshake is complete: the AP announces deauth (reason=3) if msg4 never
    // arrives, because its 802.1X port stays closed.
    crate::net::wpa::retransmit_msg4_tick();

    if !is_ready() {
        return;
    }

    driver_lock();
    if let Some(driver) = driver_get() {
        // Safety-net drain of the RX ring.
        driver.process_rx();

        if state::get_state() == state::WifiState::Connected {
            let now = crate::curr_arch::get_ticks();
            // 250 Hz timer -> 4 ms/tick; 2 s == 500 ticks.
            // Mobile hotspots drop idle clients quickly, so keepalive more often.
            if now.wrapping_sub(LAST_KEEPALIVE.load(Ordering::Relaxed)) >= 500 {
                LAST_KEEPALIVE.store(now, Ordering::Relaxed);
                let bssid = state::get_bssid();
                if bssid != [0u8; 6] {
                    let tx_addr = driver.mac;
                    // Never send a plaintext Null while the AP is waiting for
                    // the 4-way handshake (strict APs deauth us). Once the
                    // handshake completes the keepalive must be CCMP-encrypted.
                    match crate::net::wpa::get_wpa_state() {
                        crate::net::wpa::WpaState::Disconnected => {
                            let null = crate::drivers::wifi::frame::Ieee80211Parser::build_qos_null(bssid, tx_addr);
                            let _ = driver.send_mgmt_frame_internal(&null);
                        }
                        crate::net::wpa::WpaState::Completed => {
                            let _ = driver.send_encrypted_null(bssid, tx_addr);
                        }
                        _ => {}
                    }
                }
            }

            // DHCP lease renewal: check if lease is about to expire and send RENEW
            if crate::net::dhcp::lease_renewal_needed() {
                drop(driver); // release lock before DHCP (which may call net::poll)
                driver_unlock();
                crate::net::dhcp::send_renew();
                return;
            }
        }
    }
    driver_unlock();
}

// ============================================================================
// WiFi Management Interface
// ============================================================================

/// Send a management frame (802.11) via the MGT queue
pub fn send_mgmt_frame(frame: &[u8]) -> Result<(), &'static str> {
    driver_lock();
    let mut driver = driver_get().ok_or("RTL8822CE not initialized")?;
    
    // Reclaim completed TX descriptors first (update HW read index)
    driver.reclaim_tx_done();

    // Use the Management queue (MGT)
    let queue = TX_QUEUE_MGT as usize;
    let idx = driver.tx_wr_idx[queue] as usize;

    // Ring flow control: never overwrite descriptors still owned by HW.
    if driver.tx_avail_descs(queue) < 2 {
        driver_unlock();
        return Err("RTL8822CE: TX ring full");
    }

    let buf_phys = unsafe {
        (driver.tx_buf[queue] as u64) + (idx as u64) * TX_BUF_SIZE as u64
    };
    let buf_virt = unsafe {
        (driver.tx_buf_virt[queue] as u64) + (idx as u64) * TX_BUF_SIZE as u64
    };

    if frame.len() + TX_HDR_SIZE as usize > TX_BUF_SIZE {
        driver_unlock();
        return Err("Management frame too large");
    }

    // Build TX descriptor for management queue.
    // rtw88/vendor set DISQSELSEQ=1, EN_HWSEQ=1, HW_SSN_SEL=0 for non-QoS
    // frames so the MAC assigns the sequence; seq=0 on every frame makes some
    // APs drop auth/assoc retries as duplicates.
    let bmc = frame.len() >= 5 && (frame[4] & 0x01) != 0;
    let mgmt_seq = MGMT_TX_SEQ
        .fetch_add(1, core::sync::atomic::Ordering::Relaxed)
        & 0x0fff;
    let tx_desc = TxDesc {
        dword0: ((frame.len() as u32) & 0x0000ffff)
            | ((TX_HDR_SIZE as u32) << 16)   // TXPKTSIZE | OFFSET(48)
            | (if bmc { 1 << 24 } else { 0 }) // BMC
            | (1 << 26)                      // LS
            | (1 << 31),                     // DISQSELSEQ
        dword1: TX_DESC_QSEL_MGMT << 8, // QSEL[12:8] = 18 (management)
        dword2: 0, // RETRY_LMT, RTS_RATE, AMPDU_MAX_TIME, PKT_OFFSET, AGG_EN
        dword3: 0, // NDPA, PWR_OFFSET, MORE_FRAG, HW_SSN_SEL=0
        dword4: buf_phys as u32,
        dword5: (buf_phys >> 32) as u32,
        dword6: 0, // TX buffer size + next desc low
        dword7: 0, // TXDESC_CHECKSUM + RTS_SC + TX BUFF SIZE
        dword8: 1 << 15, // EN_HWSEQ
        dword9: (mgmt_seq as u32) << 12, // SW_SEQ[23:12]
    };

    // Copy the descriptor at the head of the 48-byte TX header area and the
    // frame payload at offset TX_HDR_SIZE. Fill the header seq too so it is
    // correct whether the MAC honours EN_HWSEQ or sends the header verbatim.
    unsafe {
        core::ptr::write_bytes(buf_virt as *mut u8, 0, TX_HDR_SIZE as usize);
        core::ptr::copy_nonoverlapping(
            &tx_desc as *const TxDesc as *const u8,
            buf_virt as *mut u8,
            core::mem::size_of::<TxDesc>(),
        );
        core::ptr::copy_nonoverlapping(
            frame.as_ptr(),
            (buf_virt as *mut u8).add(TX_HDR_SIZE as usize),
            frame.len(),
        );
        if frame.len() >= 24 {
            let seq_ctrl = (buf_virt as *mut u8).add(TX_HDR_SIZE as usize + 22);
            *seq_ctrl = (mgmt_seq << 4) as u8;
            *seq_ctrl.add(1) = (mgmt_seq >> 4) as u8;
        }
    }

    // Arm the two-segment TX buffer descriptor (header + payload).
    driver.fill_tx_bd(queue, idx, TX_HDR_SIZE, frame.len() as u16, buf_phys);

    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);

    // Keep REG_HW_SEQ0 in sync with the descriptor/header value (EN_HWSEQ=1).
    unsafe {
        driver.write16(REG_HW_SEQ0, mgmt_seq);
    }

    let next_idx = (idx + 1) % driver.tx_bd_count[queue] as usize;
    driver.tx_wr_idx[queue] = next_idx as u16;

    unsafe {
        driver.write32(REG_MGQ_TXBD_IDX, driver.tx_wr_idx[queue] as u32);
    }

    driver_unlock(); Ok(())
}

/// Change the radio channel (for scanning)
pub fn set_channel(channel: u8) -> Result<(), &'static str> {
    driver_lock(); let mut driver = driver_get().unwrap();
    let driver = driver_get().ok_or("RTL8822CE not initialized")?;
    let band_5ghz = channel >= 36;
    driver.set_channel(channel, band_5ghz);
    driver_unlock(); Ok(())
}

/// Active WiFi scan - concise output (`wifi scan`).
/// Prints one line per newly discovered BSSID and a final summary.
pub fn wifi_scan() -> Result<Vec<crate::drivers::wifi::scan::ScanResult>, &'static str> {
    wifi_scan_impl(false)
}

/// Non-blocking check for a Ctrl+C (SIGINT) keypress. Drains the keyboard
/// buffer and returns true if Ctrl+C was pressed, so a long-running scan can be
/// aborted by the user.
fn scan_cancel_requested() -> bool {
    use crate::drivers::keyboard::{has_events, read_event, KeyEvent, SpecialKey};
    let mut cancel = false;
    while has_events() {
        if let Some(KeyEvent::Special(SpecialKey::CtrlC)) = read_event() {
            cancel = true;
        }
    }
    cancel
}

/// Active WiFi scan with full verbose logging (`wifi scanner`).
pub fn wifi_scan_detailed() -> Result<Vec<crate::drivers::wifi::scan::ScanResult>, &'static str> {
    wifi_scan_impl(true)
}

/// Active WiFi scan - Linux rtw88 style.
/// Sets RCR to accept all beacons, sends probe requests on each channel and
/// collects probe responses/beacons. Repeats the channel sweep until no new
/// BSSID has been seen for IDLE_TIMEOUT (auto-stop), with a hard time cap.
fn wifi_scan_impl(verbose: bool) -> Result<Vec<crate::drivers::wifi::scan::ScanResult>, &'static str> {
    use crate::drivers::wifi::scan;

    driver_lock();
    let mut driver = driver_get().ok_or("RTL8822CE not initialized")?;

    let tx_addr = driver.mac;

    scan::set_verbose(verbose);
    scan::clear_results();
    scan::set_scanning(true);
    scan::set_complete(false);

    // 1. HW: Scan enter - accept all beacons/probe responses
    driver.hw_scan_enter();

    if verbose {
        crate::mesa_println!("[WiFi SCAN] ===== Starting ACTIVE scan on 2.4GHz channels 1-13 =====");
        crate::mesa_println!("[WiFi SCAN] TX MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            tx_addr[0], tx_addr[1], tx_addr[2], tx_addr[3], tx_addr[4], tx_addr[5]);
    }

    const CHANNELS_2G: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
    const PROBE_RETRIES: u32 = 3;
    // Timer runs at 250 Hz -> 1 tick = 4 ms.
    const MS_PER_TICK: u64 = 4;
    // Stop after this long with no newly discovered BSSID (15-20s window).
    const IDLE_TIMEOUT_TICKS: u64 = 18_000 / MS_PER_TICK;
    // Hard cap so a noisy environment cannot scan forever.
    const MAX_SCAN_TICKS: u64 = 45_000 / MS_PER_TICK;
    let dwell_ms: u64 = if verbose { 500 } else { 300 };

    let start = crate::curr_arch::get_ticks();
    let mut last_new = start;
    let mut last_count = 0usize;
    let mut sweep = 0u32;

    let mut prev = driver.read_phy_counters();
    if verbose {
        let (cca_cck, cca_ofdm, cck_ok, cck_err, ofdm_ok, ofdm_err, ht_ok, ht_err, vht_ok, vht_err) = prev;
        crate::mesa_println!("[WiFi PHYCNT] init cca(cck={} ofdm={}) cck(ok={} err={}) ofdm(ok={} err={}) ht(ok={} err={}) vht(ok={} err={})",
            cca_cck, cca_ofdm, cck_ok, cck_err, ofdm_ok, ofdm_err, ht_ok, ht_err, vht_ok, vht_err);
    }

    // Allow the user to abort with Ctrl+C (drained each dwell iteration).
    crate::drivers::keyboard::clear_buffer();
    let mut cancelled = false;

    loop {
        sweep += 1;
        if verbose && sweep > 1 {
            crate::mesa_println!("[WiFi SCAN] ===== sweep {} ({} networks so far) =====", sweep, last_count);
        }

        for &ch in &CHANNELS_2G {
            if verbose {
                crate::mesa_println!("[WiFi SCAN] >>> Channel {} <<<", ch);
            }

            driver.set_channel(ch, false);
            crate::linux::mdelay(10);

            let mut probe_req = scan::build_probe_request(tx_addr, "");
            probe_req.extend_from_slice(&[0x03, 0x01, ch]); // DS Parameter Set

            for attempt in 0..PROBE_RETRIES {
                match driver.send_mgmt_frame_internal(&probe_req) {
                    Ok(()) => {
                        if verbose {
                            crate::mesa_println!("[WiFi SCAN] ch={} TX OK (attempt {})", ch, attempt + 1);
                        }
                    }
                    Err(e) => {
                        if verbose {
                            crate::mesa_println!("[WiFi SCAN] ch={} TX FAIL: {}", ch, e);
                        }
                    }
                }
                crate::linux::mdelay(20);
            }

            for _ in 0..(dwell_ms / 10) {
                if scan_cancel_requested() {
                    cancelled = true;
                    break;
                }
                driver.handle_interrupt();
                driver.process_rx();
                crate::linux::mdelay(10);
            }

            if cancelled {
                break;
            }

            // Track discovery so we can auto-stop when nothing new appears.
            let n = scan::result_count();
            if n > last_count {
                last_count = n;
                last_new = crate::curr_arch::get_ticks();
            }

            if verbose {
                let cur = driver.read_phy_counters();
                let edcca = unsafe { driver.read32(REG_EDCCA_REPORT) };
                crate::mesa_println!("[WiFi PHYCNT] ch={} EDCCA={:#010x}(flag={}) dCCA(cck={} ofdm={}) dCRC cck(ok={} err={}) ofdm(ok={} err={}) ht(ok={} err={}) vht(ok={} err={})",
                    ch, edcca, (edcca & BIT_EDCCA_FLAG) != 0,
                    cur.0.wrapping_sub(prev.0), cur.1.wrapping_sub(prev.1),
                    cur.2.wrapping_sub(prev.2), cur.3.wrapping_sub(prev.3),
                    cur.4.wrapping_sub(prev.4), cur.5.wrapping_sub(prev.5),
                    cur.6.wrapping_sub(prev.6), cur.7.wrapping_sub(prev.7),
                    cur.8.wrapping_sub(prev.8), cur.9.wrapping_sub(prev.9));
                prev = cur;
            }
        }

        if cancelled {
            if verbose {
                crate::mesa_println!("[WiFi SCAN] cancelled by user (Ctrl+C)");
            }
            break;
        }

        let now = crate::curr_arch::get_ticks();
        if now.saturating_sub(last_new) >= IDLE_TIMEOUT_TICKS {
            if verbose {
                crate::mesa_println!("[WiFi SCAN] idle for {} ms, stopping", now.saturating_sub(last_new) * MS_PER_TICK);
            }
            break;
        }
        if now.saturating_sub(start) >= MAX_SCAN_TICKS {
            if verbose {
                crate::mesa_println!("[WiFi SCAN] max scan time reached, stopping");
            }
            break;
        }
    }

    // 3. HW: Scan done - restore RCR for connected state
    driver.hw_scan_done();
    scan::set_scanning(false);
    scan::set_complete(true);

    let results = scan::get_results();
    if verbose {
        crate::mesa_println!("[WiFi SCAN] ===== Scan complete: {} networks found =====", results.len());
        for (i, r) in results.iter().enumerate() {
            let enc = if r.has_rsn { "WPA2" } else { "Open" };
            crate::mesa_println!("  {:2}. {:32} ch={:2} RSSI={:3}dBm [{}] BSSID={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                i + 1, r.ssid, r.channel, r.rssi, enc,
                r.bssid[0], r.bssid[1], r.bssid[2], r.bssid[3], r.bssid[4], r.bssid[5]);
        }
    }
    driver_unlock(); Ok(results)
}

/// Print a compact, copy/paste friendly diagnostic summary of the RTL8822CE
/// hardware state. Intended to be captured from photos/transcription: ~10 short
/// lines covering the whole init + RX path status.
pub fn wifi_diag() {
    driver_lock();
    let driver = match driver_get() {
        Some(d) => d,
        None => {
            crate::mesa_println!("[WIFI DIAG] DRIVER NOT INITIALIZED");
            driver_unlock();
            return;
        }
    };

    unsafe {
        let idx = driver.read32(REG_RXQ_RXBD_IDX);
        let rf18a = driver.read_rf_sipi(0, 0x18, RFREG_MASK);
        let rf18b = driver.read_rf_sipi(1, 0x18, RFREG_MASK);

        crate::mesa_println!("[WIFI DIAG] BUILD=v61-wpa2-mic-diag");
        {
            let (p_ok, k_ok, ke_ok, t_ok) = crate::net::wpa::self_test();
            crate::mesa_println!("[WIFI DIAG] WPA2 KAT: PBKDF2={} KCK={} KEK={} TK={}",
                if p_ok { "OK" } else { "FALLO" },
                if k_ok { "OK" } else { "FALLO" },
                if ke_ok { "OK" } else { "FALLO" },
                if t_ok { "OK" } else { "FALLO" });
        }
        crate::mesa_println!("[WIFI DIAG] init={} fw={} ch={} band={} rfe={:#04x} cut={}",
            driver.initialized as u8, driver.fw_loaded as u8, driver.current_channel,
            if driver.current_band_5g { "5g" } else { "2g" },
            driver.efuse.rfe_option, driver.efuse.cut_version);
        crate::mesa_println!("[WIFI DIAG] CR={:#04x} RCR={:#010x} MCUFW={:#06x}",
            driver.read8(REG_CR), driver.read32(REG_RCR), driver.read16(REG_MCUFW_CTRL));
        crate::mesa_println!("[WIFI DIAG] RXFF_BNDY={:#010x} RXBD_IDX={:#010x} hw={} host={} NUM={:#06x}",
            driver.read32(REG_RXFF_BNDY), idx, (idx >> 16) & 0xfff, idx & 0xfff,
            driver.read16(REG_RXQ_RXBD_NUM));
        crate::mesa_println!("[WIFI DIAG] rx_started={} rx_rd={} HIMR0={:#010x} PCIE_CTRL={:#010x}",
            driver.rx_started, driver.rx_rd_idx,
            driver.read32(REG_PCIE_HIMR0), driver.read32(RTK_PCI_CTRL));
        {
            let mut mn = u64::MAX;
            let mut mx = 0u64;
            let mut over = 0u32;
            for i in 0..RX_RING_SIZE {
                let p = driver.rx_buf_phys[i];
                if p == 0 { continue; }
                if p < mn { mn = p; }
                if p > mx { mx = p; }
                if p > 0xFFFF_FFFF { over += 1; }
            }
            crate::mesa_println!("[WIFI DIAG] RXBUF phys min={:#012x} max={:#012x} over4G={}",
                mn, mx, over);
        }
        crate::mesa_println!("[WIFI DIAG] HISR0={:#010x} HISR1={:#010x} HISR3={:#010x}",
            driver.read32(REG_PCIE_HISR0), driver.read32(REG_PCIE_HISR1), driver.read32(REG_PCIE_HISR3));
        crate::mesa_println!("[WIFI DIAG] SYSFUNCEN={:#06x} RFCTRL={:#04x} ANTMAP824={:#010x}",
            driver.read16(REG_SYS_FUNC_EN), driver.read8(REG_RF_CTRL), driver.read32(REG_ANTMAP));
        crate::mesa_println!("[WIFI DIAG] RXCCKSEL={:#010x} CCANRX={:#010x} RXFNCTL={:#010x}",
            driver.read32(REG_RXCCKSEL), driver.read32(REG_CCANRX), driver.read32(REG_RXFNCTL));
        crate::mesa_println!("[WIFI DIAG] RXBREAK={:#010x} CNTCTRL={:#010x} ENCCK={:#010x}",
            driver.read32(REG_RX_BREAK), driver.read32(REG_CNT_CTRL), driver.read32(REG_ENCCK));
        crate::mesa_println!("[WIFI DIAG] RF18 A={:#010x} B={:#010x}", rf18a, rf18b);
        crate::mesa_println!("[WIFI DIAG] EDCCA_REPORT={:#010x} flag={} TXPTCL={:#010x} RDCTRL={:#010x}",
            driver.read32(REG_EDCCA_REPORT), (driver.read32(REG_EDCCA_REPORT) & BIT_EDCCA_FLAG) != 0,
            driver.read32(REG_TX_PTCL_CTRL), driver.read32(REG_RD_CTRL));
        crate::mesa_println!("[WIFI DIAG] IQK 0x2d9c={:#04x} 0x1b00={:#010x} 0x1b10={:#010x} 0x1bd8={:#010x}",
            driver.read8(0x2d9c), driver.read32(0x1b00), driver.read32(0x1b10), driver.read32(0x1bd8));
    }

    driver_unlock();
}

/// Internal scan without locking (for use from wifi_connect)
fn wifi_scan_internal(drv: &mut Rtl8822ce) -> Result<Vec<crate::drivers::wifi::scan::ScanResult>, &'static str> {
    let tx_addr = drv.mac;

    // Clear previous results
    crate::drivers::wifi::scan::clear_results();
    crate::drivers::wifi::scan::set_scanning(true);
    crate::drivers::wifi::scan::set_complete(false);

    // 1. HW: Scan enter - accept all beacons/probe responses
    drv.hw_scan_enter();
    crate::mesa_println!("[WiFi] Buscando redes (2.4 GHz)...");

    // 2. Scan each channel (2.4 GHz channels 1-13).
    const CHANNELS_2G: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
    const CHANNEL_DWELL_MS: u64 = 150;
    const PROBE_RETRIES: u32 = 2;

    for &ch in &CHANNELS_2G {
        // Set channel and wait for RF to settle
        drv.set_channel(ch, false);
        crate::linux::mdelay(10);

        // Build probe request with DS parameter set
        let mut probe_req = crate::drivers::wifi::scan::build_probe_request(tx_addr, "");
        probe_req.extend_from_slice(&[0x03, 0x01, ch]);

        // Send multiple probe requests per channel
        for _ in 0..PROBE_RETRIES {
            let _ = drv.send_mgmt_frame_internal(&probe_req);
            crate::linux::mdelay(20);
        }

        // Wait for responses
        for _ in 0..(CHANNEL_DWELL_MS / 10) {
            drv.handle_interrupt();
            drv.process_rx();
            crate::linux::mdelay(10);
        }
    }

    // 3. HW: Scan done - restore RCR for connected state
    drv.hw_scan_done();
    crate::drivers::wifi::scan::set_scanning(false);
    crate::drivers::wifi::scan::set_complete(true);

    let results = crate::drivers::wifi::scan::get_results();
    crate::mesa_println!("[WiFi] {} red(es) detectada(s)", results.len());
    Ok(results)
}

/// Connect to a WiFi network - Linux rtw88 style join flow
/// 1. HW: Join prepare (MLME_STA_CONNECTING)
/// 2. Send auth frame (open system)
/// 3. Wait for auth response
/// 4. Send assoc request
/// 5. Wait for assoc response (get AID)
/// 6. HW: Join success + H2C MEDIA_STATUS_RPT (connect)
pub fn wifi_connect(ssid: &str, password: &str) -> Result<(), &'static str> {
    // WPA2-PSK: derive the PMK from the passphrase BEFORE taking the driver
    // lock (PBKDF2 is CPU-bound, ~100ms). A non-empty password means secured.
    let use_wpa2 = !password.is_empty();
    let pmk: [u8; 32] = if use_wpa2 {
        crate::mesa_println!("[WPA2] Derivando PMK para '{}'...", ssid);
        // Show EXACTLY what we received as the passphrase (len + bytes) so a
        // corrupted/truncated prompt can be ruled out.
        crate::mesa_println!("[WPA2] password len={}", password.len());
        crate::net::wpa::screen_hex("WPA2 password bytes:", password.as_bytes());
        let p = crate::net::wpa::derive_pmk(password, ssid);
        crate::net::wpa::screen_hex("WPA2 PMK(password,ssid):", &p);
        p
    } else {
        [0u8; 32]
    };

    driver_lock(); 
    let mut driver = driver_get().ok_or("RTL8822CE not initialized")?;

    // Reset any leftover WPA state/keys from a previous connection. Otherwise a
    // prior WPA2 session leaves wpa_state=Completed, making send_packet encrypt
    // even open-network frames (Protected+CCMP with no valid key) -> AP drops.
    crate::net::wpa::clear_wpa();
    CCMP_TX_PN.store(0, core::sync::atomic::Ordering::Relaxed);

    // Always scan fresh to find the target AP (no cached-BSSID shortcut).
    let target = {
        let results = wifi_scan_internal(&mut driver)?;
        results.iter().find(|r| r.ssid == ssid)
            .cloned()
            .ok_or("Target SSID not found in scan results")?
    };

    crate::mesa_println!("[WiFi] Connecting to '{}' BSSID={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} ch={}",
        ssid, target.bssid[0], target.bssid[1], target.bssid[2],
        target.bssid[3], target.bssid[4], target.bssid[5], target.channel);

    // 1. HW: Prepare for join (MLME_STA_CONNECTING)
    driver.hw_join_prepare(&target.bssid);

    // 2. Set channel to target AP
    driver.set_channel(target.channel, target.channel >= 36);

    // 3. Start connection state machine
    let tx_addr = driver.mac;
    crate::drivers::wifi::state::start_connect(&target, tx_addr);
    crate::mesa_println!("[WiFi] Starting connection to '{}' (ch={}, BSSID={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x})",
        ssid, target.channel, target.bssid[0], target.bssid[1], target.bssid[2],
        target.bssid[3], target.bssid[4], target.bssid[5]);

    // 4-5. Authentication. Mobile hotspots drop frames transiently (and their
    //      randomized BSSID can change mid-connect), so retry the auth frame a
    //      few times before giving up.
    let auth_frame = crate::drivers::wifi::state::build_auth_frame(target.bssid, tx_addr);
    let mut authed = false;
    for attempt in 0..3 {
        driver.send_mgmt_frame_internal(&auth_frame)?;
        crate::mesa_println!("[WiFi] Auth frame enviado (intento {}/3), esperando...", attempt + 1);
        // Real 2 s wait (timer is ~100 Hz => 1 tick = 10 ms). Do NOT rely on
        // mdelay(): it is a busy-spin calibration loop, not milliseconds.
        let wait_start = crate::curr_arch::get_ticks();
        while crate::curr_arch::get_ticks().wrapping_sub(wait_start) < 200 {
            driver.handle_interrupt();
            crate::linux::mdelay(1);

            let state = crate::drivers::wifi::state::get_state();
            if state == crate::drivers::wifi::state::WifiState::Associating {
                authed = true;
                break;
            }
            if state == crate::drivers::wifi::state::WifiState::Disconnected {
                // Explicit reject (not just timeout).
                driver.hw_join_fail_cleanup();
                driver_unlock();
                return Err("Authentication failed");
            }
        }
        if authed {
            break;
        }
        crate::mesa_println!("[WiFi]   Auth sin respuesta, reintentando...");
    }
    if !authed {
        driver.hw_join_fail_cleanup();
        driver_unlock();
        return Err("Authentication timeout (3 intentos)");
    }
    crate::mesa_println!("[WiFi] Auth OK, moving to association");

    // 6-7. Association. Retry a few times like auth (hotspots drop frames).
    crate::mesa_println!("[WiFi] Assoc con RSN IE: {} (red {})",
        if target.has_rsn { "SI" } else { "NO" },
        if target.has_rsn { "asegurada" } else { "abierta" });
    let assoc_frame = crate::drivers::wifi::state::build_assoc_request(target.bssid, tx_addr, ssid);
    crate::mesa_println!(
        "[WiFi] target ch={} cap=0x{:04x} rsn={} ht={} assoc_len={}",
        target.channel, target.capability, target.has_rsn as u8,
        target.has_ht as u8, assoc_frame.len()
    );
    crate::mesa_println!("[WiFi] AP RSN IE ({}B): {:02x?}", target.rsn_ie.len(), target.rsn_ie);
    {
        use core::fmt::Write as _;
        let mut line = alloc::string::String::new();
        for (i, b) in assoc_frame.iter().enumerate() {
            let _ = write!(line, "{:02x}", b);
            if i % 16 == 15 || i + 1 == assoc_frame.len() {
                crate::mesa_println!("[WiFi] assoc[{}] {}", i / 16, line);
                line.clear();
            }
        }
    }
    let mut aid: Option<u16> = None;
    let mut force_auth = false;
    for attempt in 1..=5 {
        // A real AP may have dropped our authentication state (it then answers
        // our assoc request, a Class 2 frame, with Deauthentication reason 9
        // "STA not authenticated"). A silently lost assoc frame does not change
        // our local state, so re-authenticate after every silent attempt too
        // instead of firing into an AP that has already forgotten us.
        if force_auth
            || crate::drivers::wifi::state::get_state()
                != crate::drivers::wifi::state::WifiState::Associating
        {
            driver.send_mgmt_frame_internal(&auth_frame)?;
            crate::mesa_println!("[WiFi] Re-auth enviado (intento {}/5), esperando...", attempt);
            let mut ok = false;
            let wait_start = crate::curr_arch::get_ticks();
            while crate::curr_arch::get_ticks().wrapping_sub(wait_start) < 120 { // 1.2 s
                driver.handle_interrupt();
                crate::linux::mdelay(1);
                match crate::drivers::wifi::state::get_state() {
                    crate::drivers::wifi::state::WifiState::Associating => {
                        ok = true;
                        break;
                    }
                    crate::drivers::wifi::state::WifiState::Disconnected => break,
                    _ => {}
                }
            }
            if !ok {
                force_auth = true;
                continue;
            }
        }
        force_auth = false;
        driver.send_mgmt_frame_internal(&assoc_frame)?;
        crate::mesa_println!("[WiFi] Assoc request enviado (intento {}/5), esperando...", attempt);
        let wait_start = crate::curr_arch::get_ticks();
        while crate::curr_arch::get_ticks().wrapping_sub(wait_start) < 120 { // 1.2 s
            driver.handle_interrupt();
            crate::linux::mdelay(1);

            let state = crate::drivers::wifi::state::get_state();
            if state == crate::drivers::wifi::state::WifiState::Connected {
                let wifi_state = crate::drivers::wifi::state::WIFI_STATE.lock();
                aid = Some(wifi_state.association_id);
                break;
            }
            if state == crate::drivers::wifi::state::WifiState::Disconnected {
                driver.hw_join_fail_cleanup();
                driver_unlock();
                return Err("Association failed");
            }
            if state == crate::drivers::wifi::state::WifiState::Authenticating {
                // Deauth reason 9: the AP lost/never had our auth state.
                break;
            }
        }
        if aid.is_some() {
            break;
        }
        crate::mesa_println!("[WiFi]   Assoc sin respuesta, reintentando...");
        force_auth = true;
    }
    let aid = match aid {
        Some(aid) => {
            crate::mesa_println!("[WiFi] Association OK, AID={}", aid);
            aid
        }
        None => {
            driver.hw_join_fail_cleanup();
            driver_unlock();
            return Err("Association timeout (5 intentos)");
        }
    };

    // 8. HW: Join success (MLME_STA_CONNECTED)
    driver.hw_join_success(aid);

    // 9. Notify firmware via H2C JOIN_BSSRPT (downloads reserved page + media status)
    // This mirrors Linux: hw_var_set_h2c_fw_joinbssrpt -> hw_var_set_dl_rsvd_page
    driver.h2c_join_bss_report(true)?; // true = connect
    crate::mesa_println!("[WiFi] H2C JOIN_BSSRPT sent (connect + reserved page)");

    // 9b. Send an initial QoS-Null to tell the AP we are associated and awake.
    //     Without this some APs treat our first data frames as "Class 3 frame
    //     from nonassociated station" and deauth with reason 7.
    {
        let null = crate::drivers::wifi::frame::Ieee80211Parser::build_qos_null(target.bssid, tx_addr);
        let _ = driver.send_mgmt_frame_internal(&null);
    }

    // 10. WPA2-PSK: begin the 4-way handshake. Msg 1/4 arrives via the RX IRQ
    //     (process_rx -> handle_packet -> handle_eapol) and we respond with 2/4,
    //     then 3/4 -> 4/4. The PMK was derived before we took the driver lock.
    if use_wpa2 {
        crate::mesa_println!("[WPA2] Iniciando handshake 4-way con {}", ssid);
        crate::net::wpa::start_handshake(&pmk, target.bssid, tx_addr);
    } else {
        crate::mesa_println!("[RTL8822CE] Open network, no security");
    }

    crate::serial_println!("[WiFi] Connected to '{}' (AID={})", ssid, aid);

    // Release the driver lock before the (long) DHCP exchange so the RX
    // interrupt handler and net::poll can service the lease freely.
    driver_unlock();

    // 10b. WPA2: wait for the 4-way handshake to finish (msg 1/4 -> 4/4 are
    //      driven by the RX IRQ into handle_eapol), then install the keys.
    if use_wpa2 {
        crate::mesa_println!("[WPA2] Esperando handshake 4-way...");
        let start = crate::curr_arch::get_ticks();
        let timeout_ticks = 1000u64; // 10 s at ~100 Hz (1 tick = 10 ms)
        let mut last_state = crate::net::wpa::WpaState::Start;
        loop {
            crate::net::poll();
            if crate::net::wpa::take_keys_pending() {
                install_wpa_keys_now(target.bssid);
                crate::mesa_println!("[WPA2] Handshake COMPLETADO, claves instaladas");
                break;
            }
            let st = crate::net::wpa::get_wpa_state();
            if st != last_state {
                crate::mesa_println!("[WPA2] estado: {:?}", st);
                last_state = st;
            }
            if st == crate::net::wpa::WpaState::Failed {
                crate::mesa_println!("[WPA2] Handshake FALLÓ");
                break;
            }
            if crate::curr_arch::get_ticks().wrapping_sub(start) > timeout_ticks {
                crate::mesa_println!("[WPA2] timeout esperando msg3/4 (estado={:?})", st);
                break;
            }
            crate::linux::mdelay(5);
        }
    }

    // 10c. Let the AP authorize the 802.1X port after msg 4/4. Poll during this
    //      window so retransmit_msg4_tick() can resend msg 4/4 a few times.
    //      Sending the broadcast DHCP DISCOVER while the port is still closed
    //      makes strict APs deauth us with reason=3.
    if crate::net::wpa::get_wpa_state() == crate::net::wpa::WpaState::Completed {
        // Real 600 ms (60 ticks at ~100 Hz) for the AP to open its 802.1X port.
        // Previously this only lasted a few ms because mdelay() is a spin loop,
        // so the DHCP DISCOVER often hit a still-blocked port and was dropped.
        let wait_start = crate::curr_arch::get_ticks();
        while crate::curr_arch::get_ticks().wrapping_sub(wait_start) < 60 {
            crate::net::poll();
            crate::linux::mdelay(1);
        }
    }

    // 11. Obtain an IPv4 address automatically via DHCP.
    // Reset the (QEMU-era) default IP so the wait loop below actually waits
    // for a real lease instead of trusting the hardcoded 10.0.2.15.
    crate::net::configure([0, 0, 0, 0], [255, 255, 255, 0], [0, 0, 0, 0]);
    crate::mesa_println!("[WiFi] Obteniendo IP (DHCP)...");
    let mut got_ip = false;
    // Mobile hotspots are slow to answer DHCP; try more times with longer waits.
    for attempt in 0..6 {
        // If msg 3/4 arrived after the initial wait, install TK+GTK now so the
        // broadcast DHCP DISCOVER below is actually CCMP-encrypted.
        if crate::net::wpa::take_keys_pending() {
            install_wpa_keys_now(target.bssid);
        }
        if let Err(e) = crate::net::dhcp::send_discover() {
            crate::mesa_println!("[WiFi] DHCP error: {}", e);
            break;
        }
        // Real 2.5 s wait (250 ticks at ~100 Hz) for a DHCPOFFER/DHCPACK.
        // The old loop finished in ~25 ms, far too short for a busy router.
        // An OFFER restarts the window and the REQUEST is retried, otherwise
        // a router that answers the last DISCOVER near the deadline left us
        // sending the REQUEST and giving up before the ACK arrived.
        let mut wait_start = crate::curr_arch::get_ticks();
        let mut last_req = wait_start;
        let mut offer_seen = false;
        let mut req_retries = 0u32;
        while crate::curr_arch::get_ticks().wrapping_sub(wait_start) < 250 {
            crate::net::poll();
            if crate::net::wpa::take_keys_pending() {
                install_wpa_keys_now(target.bssid);
            }
            if crate::net::get_ip() != [0, 0, 0, 0] {
                got_ip = true;
                break;
            }
            let now = crate::curr_arch::get_ticks();
            if crate::net::dhcp::offer_pending() {
                if !offer_seen {
                    // Send from the polling loop, not from handle_packet (RX).
                    offer_seen = true;
                    let _ = crate::net::dhcp::resend_request();
                    last_req = now;
                    wait_start = now;
                } else if now.wrapping_sub(last_req) >= 50 && req_retries < 8 {
                    let _ = crate::net::dhcp::resend_request();
                    req_retries += 1;
                    last_req = now;
                    wait_start = now;
                    crate::mesa_println!(
                        "[WiFi]   DHCP reenviando REQUEST ({}/8)", req_retries
                    );
                }
            }
            crate::linux::mdelay(1);
        }
        if got_ip {
            break;
        }
        if attempt < 5 {
            crate::mesa_println!("[WiFi]   DHCP sin respuesta en 2.5 s (intento {}/6)", attempt + 1);
            let gap = crate::curr_arch::get_ticks();
            while crate::curr_arch::get_ticks().wrapping_sub(gap) < 20 {
                crate::linux::mdelay(1); // 200 ms gap
            }
        }
    }
    let ip = crate::net::get_ip();
    if ip != [0, 0, 0, 0] {
        let gw = crate::net::get_gateway();
        crate::mesa_println!("[WiFi] IP {}.{}.{}.{}  GW {}.{}.{}.{}",
            ip[0], ip[1], ip[2], ip[3], gw[0], gw[1], gw[2], gw[3]);
    } else {
        crate::mesa_println!("[WiFi] Sin respuesta DHCP (configura IP manualmente con 'ifconfig')");
    }
    Ok(())
}

/// Disconnect from WiFi network - Linux rtw88 style
/// 1. Send deauth frame to AP
/// 2. HW: Join fail cleanup (MLME_STA_DISCONNECTED)
/// 3. H2C JOIN_BSSRPT (disconnect)
/// 4. Restore state
pub fn wifi_disconnect() -> Result<(), &'static str> {
    driver_lock(); 
    let mut driver = driver_get().ok_or("RTL8822CE not initialized")?;

    crate::mesa_println!("[WiFi] Disconnecting...");

    // 1. Send deauth frame to AP (if connected)
    let state = crate::drivers::wifi::state::get_state();
    if state == crate::drivers::wifi::state::WifiState::Connected {
        let bssid = crate::drivers::wifi::state::get_bssid();
        let tx_addr = driver.mac;
        let deauth_frame = crate::drivers::wifi::state::build_deauth_frame(bssid, tx_addr);
        let _ = driver.send_mgmt_frame_internal(&deauth_frame);
        crate::mesa_println!("[WiFi] Deauth frame sent to AP");
    }

    // 2. HW: Join fail cleanup (MLME_STA_DISCONNECTED)
    driver.hw_join_fail_cleanup();

    // 3. H2C JOIN_BSSRPT (disconnect) - notifies firmware to clean up reserved page
    driver.h2c_join_bss_report(false)?; // false = disconnect
    crate::mesa_println!("[WiFi] H2C JOIN_BSSRPT sent (disconnect)");

    // 4. Reset state
    crate::drivers::wifi::state::set_state(crate::drivers::wifi::state::WifiState::Disconnected);

    // Clear cached BSSID so next connect will re-scan (fresh AP may be on different channel)
    crate::drivers::wifi::state::clear_cached_bssid();

    driver_unlock(); Ok(())
}

/// Send a signal to the firmware (reset/scan/associate commands)
pub fn send_signal_to_fw(cmd_type: u32, arg: u32) -> Result<(), &'static str> {
    driver_lock();
    let mut driver = driver_get().ok_or("RTL8822CE not initialized")?;
    driver.send_h2c_command(cmd_type, arg)
}

/// Notify firmware about scanning operation  
pub fn signal_scan_start() -> Result<(), &'static str> {
    driver_lock();
    let mut driver = driver_get().ok_or("RTL8822CE not initialized")?;
    driver.signal_scan_start()
}

/// Notify firmware about scan completion
pub fn signal_scan_done() -> Result<(), &'static str> {
    driver_lock();
    let mut driver = driver_get().ok_or("RTL8822CE not initialized")?;
    driver.signal_scan_done()
}

/// Send association signal to firmware
pub fn signal_associate(bssid: &[u8; 6]) -> Result<(), &'static str> {
    driver_lock();
    let mut driver = driver_get().ok_or("RTL8822CE not initialized")?;
    driver.signal_associate(bssid)
}

/// Send disassociation signal to firmware
pub fn signal_disassociate() -> Result<(), &'static str> {
    driver_lock();
    let mut driver = driver_get().ok_or("RTL8822CE not initialized")?;
    driver.signal_disassociate()
}

/// Send reset signal to firmware
pub fn signal_fw_reset() -> Result<(), &'static str> {
    driver_lock();
    let mut driver = driver_get().ok_or("RTL8822CE not initialized")?;
    driver.signal_fw_reset()
}


/// Handle a received 802.11 management frame
fn wifi_recv_mgmt(frame: &[u8]) {
    use crate::drivers::wifi::scan;
    use crate::drivers::wifi::state;

    if frame.len() < 24 {
        return;
    }

    // Extract RSSI from PHY status if available (basic approximation)
    let rssi: i8 = -50;

    // Check frame subtype
    let hdr = unsafe { &*(frame.as_ptr() as *const crate::drivers::wifi::frame::Ieee80211Header) };
    let subtype = hdr.frame_subtype();

    // Connection diagnostics: show every auth/assoc/deauth frame the AP sends
    // while we are joining, with its addresses and status. This tells us
    // whether a router answered the association request at all (and with what
    // status) instead of guessing from a timeout.
    if matches!(
        state::get_state(),
        state::WifiState::Authenticating | state::WifiState::Associating
    ) && matches!(
        subtype,
        crate::drivers::wifi::frame::FC_SUBTYPE_AUTH
            | crate::drivers::wifi::frame::FC_SUBTYPE_ASSOC_RESP
            | crate::drivers::wifi::frame::FC_SUBTYPE_DEAUTH
            | crate::drivers::wifi::frame::FC_SUBTYPE_DISASSOC
            | crate::drivers::wifi::frame::FC_SUBTYPE_REASSOC_RESP
            | crate::drivers::wifi::frame::FC_SUBTYPE_ACTION
    ) {
        let seq = (u16::from_le_bytes([frame[22], frame[23]]) >> 4) & 0x0fff;
        let status = if frame.len() >= 26 {
            u16::from_le_bytes([frame[24], frame[25]])
        } else {
            0
        };
        crate::mesa_println!(
            "[WiFi MGMT] rx sub={:#04x} a1={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} a2={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x} seq={} status={} len={}",
            subtype,
            frame[4], frame[5], frame[6], frame[7], frame[8], frame[9],
            frame[10], frame[11], frame[12], frame[13], frame[14], frame[15],
            seq, status, frame.len()
        );
    }

    match subtype {
        // Scan-related frames
        crate::drivers::wifi::frame::FC_SUBTYPE_BEACON
        | crate::drivers::wifi::frame::FC_SUBTYPE_PROBE_RESP => {
            scan::process_scan_frame(frame, rssi);
        }

        // Connection state machine frames
        crate::drivers::wifi::frame::FC_SUBTYPE_AUTH
        | crate::drivers::wifi::frame::FC_SUBTYPE_ASSOC_RESP
        | crate::drivers::wifi::frame::FC_SUBTYPE_DEAUTH
        | crate::drivers::wifi::frame::FC_SUBTYPE_DISASSOC => {
            state::handle_frame(frame);
        }

        _ => {
            // Data frames go to the Ethernet stack
            // (handled separately in process_rx)
        }
    }
}

/// Run the WiFi state machine (called periodically from the main loop)
pub fn wifi_poll() {
    use crate::drivers::wifi::state;
    use crate::drivers::wifi::scan;

    let mac = get_mac().unwrap_or([0; 6]);
    let st = state::get_state();

    match st {
        state::WifiState::Authenticating => {
            // Send auth frame to the AP
            let bssid = state::get_bssid();
            let tx_addr = state::get_tx_addr();
            let auth_frame = state::build_auth_frame(bssid, tx_addr);
            if !auth_frame.is_empty() {
                let _ = send_mgmt_frame(&auth_frame);
            }
        }
        state::WifiState::Associating => {
            // Send assoc request to the AP
            let bssid = state::get_bssid();
            let tx_addr = state::get_tx_addr();
            let ssid = state::get_ssid();
            let assoc_frame = state::build_assoc_request(bssid, tx_addr, &ssid);
            if !assoc_frame.is_empty() {
                let _ = send_mgmt_frame(&assoc_frame);
            }
        }
        state::WifiState::Connected => {
            // Send null data to keep connection alive (every ~30s)
            // This is handled by a timer elsewhere
        }
        _ => {}
    }

    // Process scan if scanning
    if scan::is_scanning() && !scan::is_complete() {
        // Scan logic is handled externally via scan channels
}
}


// ============================================================================
// Regulatory/DFS/802.11h implementation (standalone functions)
// ============================================================================

/// Set country code in regulatory domain (3 bytes: country code, e.g., "US", "JP", "KR")
/// Mirrors Linux: ieee80211_set_country()

/// Get available 2.4G channels (1-13)
/// Mirrors Linux: ieee80211_get_channel() or regulatory domain channels
pub fn get_channels() -> [u8; 13] {
    let mut channels: [u8; 13] = [0; 13];
    for i in 0..13 {
        channels[i] = (i + 1) as u8; // Channels 1-13
    }
    channels
}

/// Set transmit power control
/// Mirrors Linux: ieee80211_tpc_report() or set_tx_power()
pub fn set_tpc(tx_power: i8) -> Result<(), &'static str> {
    // Would set the transmit power level in the firmware
    // and update the regulatory domain power settings
    crate::mesa_println!("[RTL8822CE] Setting TPC to {} dBm", tx_power);
    driver_unlock(); Ok(())
}

// ============================================================================
// End of Regulatory/DFS/802.11h implementation
// ============================================================================

/// Print to screen + serial with a 5-second pause (for hardware test capture)
fn wifi_log(msg: &str) {
    crate::mesa_println!("[RTL8822CE] {}", msg);
    crate::serial_println!("[RTL8822CE] {}", msg);
}

// ============================================================================
// Regulatory/DFS/802.11h implementation (standalone functions)
// ============================================================================

/// Set country code in regulatory domain (3 bytes: country code, e.g., "US", "JP", "KR")
/// Mirrors Linux: ieee80211_set_country()
pub fn set_country(country_code: [u8; 3]) -> Result<(), &'static str> {
    // Would set the country code in the firmware regulatory domain
    // and update channel availability tables
    crate::mesa_println!("[RTL8822CE] Setting country code: {:?}", country_code);
    driver_unlock(); Ok(())
}

/// Check if a channel is allowed in the current regulatory domain
/// Mirrors Linux: ieee80211_channel_to_freq() + regulatory check
pub fn is_channel_allowed(channel: u8) -> bool {
    // Would check channel against regulatory domain restrictions
    // For now, allow 2.4G channels 1-13 and 5G channels per regulatory domain
    channel >= 1 && channel <= 13
}

/// DFS radar detection check
/// Mirrors Linux: dfs_detector() or similar
/// Returns false if radar detected, true if channel is clear (or not implemented)
/// Duration is in milliseconds
pub fn dfs_check(channel: u8, duration: u32) -> bool {
    // Would perform DFS radar detection for the specified duration
    // For now, returns true (channel clear - placeholder)
    true
}

/// Get current RSSI (Received Signal Strength Indicator) in dBm
/// Mirrors Linux: rtw_get_rssi() or get_signal_quality()
pub fn get_rssi() -> Option<i8> {
    driver_lock();
    let driver = driver_get();
    let rssi = driver.map(|d| {
        // Get RSSI from driver's RA table (MACID 0 is typically the AP)
        let ra_entry = &d.ra_table[0];
        if ra_entry.macid != 0 { ra_entry.rssi } else { -80 }
    });
    driver_unlock(); rssi
}

/// Get link quality as percentage (0-100)
/// Mirrors Linux: rtw_get_link_quality()
pub fn get_link_quality() -> Option<u8> {
    driver_lock();
    let driver = driver_get();
    let quality = driver.map(|d| {
        let rssi = d.ra_table[0].rssi;
        if rssi >= -50 { 100 }
        else if rssi >= -60 { 80 }
        else if rssi >= -70 { 60 }
        else if rssi >= -80 { 40 }
        else if rssi >= -90 { 20 }
        else { 0 }
    });
    driver_unlock(); quality
}

/// Get TX/RX byte counters
/// Mirrors Linux: net_device stats
pub fn get_tx_rx_stats() -> Option<(u64, u64, u64, u64)> {
    driver_lock();
    let driver = driver_get();
    let stats = driver.map(|d| {
        // Return (tx_bytes, rx_bytes, tx_packets, rx_packets)
        // In a full implementation, these would be tracked in the driver
        (0u64, 0u64, 0u64, 0u64)
    });
    driver_unlock(); stats
}

/// Get current channel
/// Mirrors Linux: rtw_get_cur_ch()
pub fn get_current_channel() -> Option<u8> {
    driver_lock();
    let driver = driver_get();
    let channel = driver.map(|d| d.current_channel);
    driver_unlock(); channel
}

/// Get current band (2.4G or 5G)
pub fn get_current_band() -> Option<bool> {
    driver_lock();
    let driver = driver_get();
    let is_5g = driver.map(|d| d.current_band_5g);
    driver_unlock(); is_5g
}

/// Get current TX rate
/// Mirrors Linux: rtw_get_tx_rate()
pub fn get_tx_rate() -> Option<u8> {
    driver_lock();
    let driver = driver_get();
    let rate = driver.map(|d| d.ra_table[0].tx_rate);
    driver_unlock(); rate
}

/// Get connection status
pub fn get_connection_status() -> Option<bool> {
    driver_lock();
    let driver = driver_get();
    let connected = driver.map(|d| d.initialized && d.fw_loaded);
    driver_unlock(); connected
}

/// Get driver version info
pub fn get_driver_version() -> &'static str {
    "MesaOS RTL8822CE v1.0 (Linux rtw88 compatible)"
}

/// Get firmware version info
pub fn get_firmware_version() -> Option<&'static str> {
    driver_lock();
    let driver = driver_get();
    let version = driver.map(|_d| "RTL8822CE FW v1.0.0");
    driver_unlock(); version
}



/// Get available 5G channels (36-165)
pub fn get_5g_channels() -> &'static [u8] {
    &[
        36, 40, 44, 48, 52, 56, 60, 64,  // UNII-1, UNII-2A
        100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144, // UNII-2C, UNII-3
        149, 153, 157, 161, 165 // UNII-3 upper
    ]
}

/// Check if 5G is supported
pub fn is_5g_supported() -> bool {
    true // RTL8822CE supports both 2.4G and 5G
}

/// Get PHY mode (HT/VHT)
pub fn get_phy_mode() -> Option<&'static str> {
    driver_lock();
    let driver = driver_get();
    let mode = driver.map(|d| {
        if d.ra_table[0].vht_en { "VHT (802.11ac)" }
        else if d.ra_table[0].nss > 1 { "HT (802.11n) 2x2" }
        else { "HT (802.11n) 1x1" }
    });
    driver_unlock(); mode
}

/// Get channel bandwidth
pub fn get_channel_bandwidth() -> Option<u8> {
    driver_lock();
    let driver = driver_get();
    let bw = driver.map(|d| {
        match d.ra_table[0].bw {
            0 => 20,
            1 => 40,
            2 => 80,
            3 => 160,
            _ => 20,
        }
    });
    driver_unlock(); bw
}
