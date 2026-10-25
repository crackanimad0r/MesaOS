//! 802.11 Frame Format Definitions
//!
//! Covers: Management frames (Beacon, Probe Req/Resp, Auth, Deauth, Assoc Req/Resp, Disassoc)
//!         Data frames (Data, QoS Data)
//!         Control frames (ACK, RTS, CTS) - minimal

use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;

/// Monotonic 802.11 sequence number for data/null frames. Shared across all
/// data TX so the AP's duplicate detector never sees two distinct MSDUs with
/// the same sequence number (which made it silently drop msg4 and DHCP).
static DATA_SEQ: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(0);

fn next_seq_ctrl() -> u16 {
    let seq = DATA_SEQ.fetch_add(1, core::sync::atomic::Ordering::Relaxed) & 0x0fff;
    seq << 4 // 12-bit sequence number, fragment number 0
}

// ============================================================================
// 802.11 Frame Control Field Bits
// ============================================================================
pub const FC_TYPE_MGMT: u16 = 0x0000;
pub const FC_TYPE_CTRL: u16 = 0x0004;
pub const FC_TYPE_DATA: u16 = 0x0008;

pub const FC_SUBTYPE_ASSOC_REQ: u16 = 0x0000;
pub const FC_SUBTYPE_ASSOC_RESP: u16 = 0x0010;
pub const FC_SUBTYPE_REASSOC_REQ: u16 = 0x0020;
pub const FC_SUBTYPE_REASSOC_RESP: u16 = 0x0030;
pub const FC_SUBTYPE_PROBE_REQ: u16 = 0x0040;
pub const FC_SUBTYPE_PROBE_RESP: u16 = 0x0050;
pub const FC_SUBTYPE_BEACON: u16 = 0x0080;
pub const FC_SUBTYPE_DISASSOC: u16 = 0x00A0;
pub const FC_SUBTYPE_AUTH: u16 = 0x00B0;
pub const FC_SUBTYPE_DEAUTH: u16 = 0x00C0;
pub const FC_SUBTYPE_ACTION: u16 = 0x00D0;

// FC flags
pub const FC_TO_DS: u16 = 0x0100;
pub const FC_FROM_DS: u16 = 0x0200;
pub const FC_MORE_FRAG: u16 = 0x0400;
pub const FC_RETRY: u16 = 0x0800;
pub const FC_POWER_MGMT: u16 = 0x1000;
pub const FC_MORE_DATA: u16 = 0x2000;
pub const FC_PROTECTED: u16 = 0x4000;
pub const FC_ORDER: u16 = 0x8000;

// ============================================================================
// 802.11 Information Element IDs
// ============================================================================
pub const IE_SSID: u8 = 0;
pub const IE_SUPPORTED_RATES: u8 = 1;
pub const IE_DS_PARAM: u8 = 3;
pub const IE_TIM: u8 = 5;
pub const IE_COUNTRY: u8 = 7;
pub const IE_BSS_LOAD: u8 = 11;
pub const IE_CHANNEL_SWITCH: u8 = 37;
pub const IE_RSN: u8 = 48;
pub const IE_EXTENDED_SUPPORTED_RATES: u8 = 50;
pub const IE_HT_CAPABILITIES: u8 = 45;
pub const IE_HT_OPERATION: u8 = 61;
pub const IE_VHT_CAPABILITIES: u8 = 191;
pub const IE_VHT_OPERATION: u8 = 192;
pub const IE_VENDOR_SPECIFIC: u8 = 221;

// ============================================================================
// Authentication Algorithm Numbers
// ============================================================================
pub const AUTH_ALGO_OPEN: u16 = 0;
pub const AUTH_ALGO_SHARED_KEY: u16 = 1;

// ============================================================================
// Authentication Sequence Numbers
// ============================================================================
pub const AUTH_SEQ_OPEN_1: u16 = 1;
pub const AUTH_SEQ_OPEN_2: u16 = 2;

// ============================================================================
// Status Codes
// ============================================================================
pub const STATUS_SUCCESS: u16 = 0;
pub const STATUS_AUTH_ALGO_NOT_SUPPORTED: u16 = 13;
pub const STATUS_ASSOC_DENIED_NO_HT: u16 = 27;

// ============================================================================
// Reason Codes
// ============================================================================
pub const REASON_UNSPECIFIED: u16 = 1;
pub const REASON_LEAVING: u16 = 3;

// ============================================================================
// Data Rates (802.11b/g/n)
// ============================================================================
pub const RATE_1M: u8 = 0x02;
pub const RATE_2M: u8 = 0x04;
pub const RATE_5M5: u8 = 0x0B;
pub const RATE_11M: u8 = 0x16;
pub const RATE_6M: u8 = 0x0C;
pub const RATE_9M: u8 = 0x12;
pub const RATE_12M: u8 = 0x18;
pub const RATE_18M: u8 = 0x24;
pub const RATE_24M: u8 = 0x30;
pub const RATE_36M: u8 = 0x48;
pub const RATE_48M: u8 = 0x60;
pub const RATE_54M: u8 = 0x6C;

// ============================================================================
// 802.11 Frame Header (24 bytes for management/data frames)
// ============================================================================
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Ieee80211Header {
    pub frame_control: u16,
    pub duration_id: u16,
    pub addr1: [u8; 6], // Receiver Address (RA)
    pub addr2: [u8; 6], // Transmitter Address (TA)
    pub addr3: [u8; 6], // BSSID / Source / Destination
    pub seq_ctrl: u16,
}

impl Ieee80211Header {
    pub fn new_mgmt(subtype: u16, _to_ap: bool, addr1: [u8; 6], addr2: [u8; 6], bssid: [u8; 6]) -> Self {
        // Management frames must always have ToDS=FromDS=0 (IEEE 802.11-2016
        // §9.2.4.1). Setting ToDS on a management frame makes it malformed and
        // the AP silently drops it (this broke auth/assoc with a timeout).
        let fc = FC_TYPE_MGMT | subtype;
        Self {
            frame_control: fc.to_le(),
            duration_id: 0,
            addr1,
            addr2,
            addr3: bssid,
            seq_ctrl: 0,
        }
    }

    pub fn new_data(to_ds: bool, from_ds: bool, addr1: [u8; 6], addr2: [u8; 6], bssid: [u8; 6]) -> Self {
        let fc = FC_TYPE_DATA | if to_ds { FC_TO_DS } else { 0 } | if from_ds { FC_FROM_DS } else { 0 };
        Self {
            frame_control: fc.to_le(),
            duration_id: 0,
            addr1,
            addr2,
            addr3: bssid,
            seq_ctrl: 0,
        }
    }

    pub fn frame_type(&self) -> u16 {
        let fc = u16::from_le(self.frame_control);
        fc & 0x000C
    }

    pub fn frame_subtype(&self) -> u16 {
        let fc = u16::from_le(self.frame_control);
        fc & 0x00F0
    }

    pub fn is_mgmt(&self) -> bool {
        self.frame_type() == FC_TYPE_MGMT
    }

    pub fn is_data(&self) -> bool {
        self.frame_type() == FC_TYPE_DATA
    }
}

// ============================================================================
// Information Element Builder
// ============================================================================
pub struct IeBuilder;

impl IeBuilder {
    pub fn add_ssid(ssid: &str) -> Vec<u8> {
        let ssid_bytes = ssid.as_bytes();
        let mut ie = Vec::with_capacity(2 + ssid_bytes.len());
        ie.push(IE_SSID);
        ie.push(ssid_bytes.len() as u8);
        ie.extend_from_slice(ssid_bytes);
        ie
    }

    pub fn add_ssid_wildcard() -> Vec<u8> {
        vec![IE_SSID, 0]
    }

    pub fn add_supported_rates() -> Vec<u8> {
        // Basic rates: 1, 2, 5.5, 11, 6, 12, 24 Mbps
        let rates = [RATE_1M, RATE_2M, RATE_5M5, RATE_11M, RATE_6M, RATE_12M, RATE_24M];
        let mut ie = Vec::with_capacity(2 + rates.len());
        ie.push(IE_SUPPORTED_RATES);
        ie.push(rates.len() as u8);
        ie.extend_from_slice(&rates);
        ie
    }

    pub fn add_ds_param(channel: u8) -> Vec<u8> {
        vec![IE_DS_PARAM, 1, channel]
    }

    pub fn add_ht_capabilities() -> Vec<u8> {
        // Minimal HT capabilities IE (26 bytes)
        let mut ie = Vec::with_capacity(28);
        ie.push(IE_HT_CAPABILITIES);
        ie.push(26);
        // HT Capabilities Info (2 bytes): LDPC + RX STBC 1
        ie.extend_from_slice(&[0x01, 0x01]);
        // A-MPDU Parameters (1 byte)
        ie.push(0x17);
        // Supported MCS Set (16 bytes) - basic MCS 0-7
        ie.extend_from_slice(&[
            0xFF, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ]);
        // HT Extended Capabilities (2 bytes)
        ie.extend_from_slice(&[0x00, 0x00]);
        // Transmit Beamforming Capabilities (4 bytes)
        ie.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        // ASEL Capabilities (1 byte)
        ie.push(0x00);
        ie
    }

    pub fn add_rsn_wpa2_psk() -> Vec<u8> {
        // Minimal RSN IE for WPA2-PSK (CCMP)
        let mut ie = Vec::with_capacity(22);
        ie.push(IE_RSN);
        ie.push(20); // length
        // RSN Version (2 bytes LE)
        ie.extend_from_slice(&[0x01, 0x00]);
        // Group Cipher Suite (4 bytes): CCMP
        ie.extend_from_slice(&[0x00, 0x0F, 0xAC, 0x04]);
        // Pairwise Cipher Suite Count (2 bytes LE)
        ie.extend_from_slice(&[0x01, 0x00]);
        // Pairwise Cipher Suite (4 bytes): CCMP
        ie.extend_from_slice(&[0x00, 0x0F, 0xAC, 0x04]);
        // AKM Suite Count (2 bytes LE)
        ie.extend_from_slice(&[0x01, 0x00]);
        // AKM Suite (4 bytes): PSK
        ie.extend_from_slice(&[0x00, 0x0F, 0xAC, 0x02]);
        // RSN Capabilities (2 bytes)
        ie.extend_from_slice(&[0x00, 0x00]);
        ie
    }

    /// Build an RSN IE that matches what the AP advertises. `ap_rsn` is the
    /// body (without the ID/length header) of the RSN IE from the beacon.
    ///
    /// WPA/WPA2-Personal (mixed mode) routers — common on 802.11n ISP gear —
    /// advertise a **TKIP group cipher** and TKIP+CCMP pairwise. A hardcoded
    /// "group CCMP" IE does not match the AP and some of them silently drop
    /// the association (or answer with Deauth reason 2). Mirrors what
    /// wpa_supplicant does: keep the AP's group cipher, offer CCMP if the AP
    /// supports it, and use the PSK AKM.
    pub fn add_rsn_from_ap(ap_rsn: &[u8]) -> Vec<u8> {
        const CCMP: [u8; 4] = [0x00, 0x0F, 0xAC, 0x04];
        const PSK: [u8; 4] = [0x00, 0x0F, 0xAC, 0x02];

        let mut group: [u8; 4] = CCMP;
        let mut pairwise: [u8; 4] = CCMP;
        let mut akm: [u8; 4] = PSK;

        if ap_rsn.len() >= 8 {
            group.copy_from_slice(&ap_rsn[2..6]);
            let pcount = u16::from_le_bytes([ap_rsn[6], ap_rsn[7]]) as usize;
            let poff = 8;
            if pcount > 0 && ap_rsn.len() >= poff + 4 * pcount {
                let mut best: Option<[u8; 4]> = None;
                for i in 0..pcount {
                    let s = &ap_rsn[poff + 4 * i..poff + 4 * i + 4];
                    let mut c = [0u8; 4];
                    c.copy_from_slice(s);
                    if c == CCMP {
                        best = Some(CCMP);
                        break;
                    }
                    if best.is_none() {
                        best = Some(c);
                    }
                }
                if let Some(b) = best {
                    pairwise = b;
                }
            }
            let aoff = poff + 4 * pcount;
            if ap_rsn.len() >= aoff + 2 {
                let acount = u16::from_le_bytes([ap_rsn[aoff], ap_rsn[aoff + 1]]) as usize;
                let alist = aoff + 2;
                if acount > 0 && ap_rsn.len() >= alist + 4 * acount {
                    let mut best: Option<[u8; 4]> = None;
                    for i in 0..acount {
                        let s = &ap_rsn[alist + 4 * i..alist + 4 * i + 4];
                        let mut c = [0u8; 4];
                        c.copy_from_slice(s);
                        if c == PSK {
                            best = Some(PSK);
                            break;
                        }
                        if best.is_none() {
                            best = Some(c);
                        }
                    }
                    if let Some(b) = best {
                        akm = b;
                    }
                }
            }
        }

        let mut ie = Vec::with_capacity(22);
        ie.push(IE_RSN);
        ie.push(20);
        ie.extend_from_slice(&[0x01, 0x00]);
        ie.extend_from_slice(&group);
        ie.extend_from_slice(&[0x01, 0x00]);
        ie.extend_from_slice(&pairwise);
        ie.extend_from_slice(&[0x01, 0x00]);
        ie.extend_from_slice(&akm);
        // Capabilities = 0: no PMF/preauth. WPA/WPA2 802.11n APs do not
        // require it, and copying the AP's bits could claim MFPR support.
        ie.extend_from_slice(&[0x00, 0x00]);
        ie
    }

    pub fn build_probe_request(bssid: [u8; 6], ssid: &str, tx_addr: [u8; 6]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(256);
        // Management frame header
        // For broadcast probe request: ToDS=0, FromDS=0 (not to specific AP)
        let hdr = Ieee80211Header::new_mgmt(FC_SUBTYPE_PROBE_REQ, false, bssid, tx_addr, bssid);
        frame.extend_from_slice(unsafe {
            core::slice::from_raw_parts(&hdr as *const Ieee80211Header as *const u8, 24)
        });
        // SSID IE
        frame.extend_from_slice(&Self::add_ssid(ssid));
        // Supported Rates IE
        frame.extend_from_slice(&Self::add_supported_rates());
        // HT Capabilities
        frame.extend_from_slice(&Self::add_ht_capabilities());
        frame
    }

    pub fn build_auth_open(seq: u16, addr1: [u8; 6], addr2: [u8; 6], bssid: [u8; 6]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(64);
        let hdr = Ieee80211Header::new_mgmt(FC_SUBTYPE_AUTH, true, addr1, addr2, bssid);
        frame.extend_from_slice(unsafe {
            core::slice::from_raw_parts(&hdr as *const Ieee80211Header as *const u8, 24)
        });
        // Auth Algorithm (2 bytes LE)
        frame.extend_from_slice(&AUTH_ALGO_OPEN.to_le_bytes());
        // Auth Sequence (2 bytes LE)
        frame.extend_from_slice(&seq.to_le_bytes());
        // Status Code (2 bytes LE). The IEEE 802.11 Authentication frame body
        // always carries this field; it is zero in a request. Removing it
        // (28-byte request) made APs drop our authentication entirely.
        frame.extend_from_slice(&STATUS_SUCCESS.to_le_bytes());
        frame
    }

    pub fn build_assoc_req(bssid: [u8; 6], tx_addr: [u8; 6], ssid: &str, listen_interval: u16, include_rsn: bool, ap_rsn: &[u8]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(256);
        let hdr = Ieee80211Header::new_mgmt(FC_SUBTYPE_ASSOC_REQ, true, bssid, tx_addr, bssid);
        frame.extend_from_slice(unsafe {
            core::slice::from_raw_parts(&hdr as *const Ieee80211Header as *const u8, 24)
        });
        // Capability Information (2 bytes): ESS + Short Slot (+ Privacy when
        // the AP is secured). RSN APs expect the Privacy bit for a protected
        // association; leaving it clear makes some ISP routers treat the
        // request as an open-network one.
        let cap: u16 = 0x0401 | if include_rsn { 0x0010 } else { 0 };
        frame.extend_from_slice(&cap.to_le_bytes());
        // Listen Interval (2 bytes LE)
        frame.extend_from_slice(&listen_interval.to_le_bytes());
        // SSID
        frame.extend_from_slice(&Self::add_ssid(ssid));
        // Supported Rates
        frame.extend_from_slice(&Self::add_supported_rates());
        // NOTE: no HT Capabilities IE. 802.11n requires WMM, and we do not
        // negotiate WMM, so several 802.11n APs drop (or deauth reason 2) an
        // assoc that advertises HT without WMM. Associating as a legacy 11g
        // STA is valid for every AP and avoids the issue.
        // RSN Information Element — required by secured APs. Mirror the AP's
        // advertised suites instead of a hardcoded CCMP-only IE: WPA/WPA2
        // mixed routers commonly use a TKIP group cipher.
        if include_rsn {
            if ap_rsn.len() >= 8 {
                frame.extend_from_slice(&Self::add_rsn_from_ap(ap_rsn));
            } else {
                frame.extend_from_slice(&Self::add_rsn_wpa2_psk());
            }
        }
        frame
    }

    pub fn build_deauth(addr1: [u8; 6], addr2: [u8; 6], bssid: [u8; 6], reason: u16) -> Vec<u8> {
        let mut frame = Vec::with_capacity(64);
        let hdr = Ieee80211Header::new_mgmt(FC_SUBTYPE_DEAUTH, true, addr1, addr2, bssid);
        frame.extend_from_slice(unsafe {
            core::slice::from_raw_parts(&hdr as *const Ieee80211Header as *const u8, 24)
        });
        // Reason Code (2 bytes LE)
        frame.extend_from_slice(&reason.to_le_bytes());
        frame
    }

    pub fn build_disassoc(addr1: [u8; 6], addr2: [u8; 6], bssid: [u8; 6], reason: u16) -> Vec<u8> {
        let mut frame = Vec::with_capacity(64);
        let hdr = Ieee80211Header::new_mgmt(FC_SUBTYPE_DISASSOC, true, addr1, addr2, bssid);
        frame.extend_from_slice(unsafe {
            core::slice::from_raw_parts(&hdr as *const Ieee80211Header as *const u8, 24)
        });
        frame.extend_from_slice(&reason.to_le_bytes());
        frame
    }

    /// Build a QoS Null frame to keep alive (acts as null data frame)
    pub fn build_null_data(bssid: [u8; 6], tx_addr: [u8; 6], ra: [u8; 6]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(64);
        // Data frame: to_DS=1, subtype=Data (0x0008)
        let fc = (FC_TYPE_DATA | FC_TO_DS).to_le();
        let mut hdr_bytes = [0u8; 24];
        hdr_bytes[0..2].copy_from_slice(&fc.to_le_bytes());
        hdr_bytes[2..8].copy_from_slice(&ra);
        hdr_bytes[8..14].copy_from_slice(&tx_addr);
        hdr_bytes[14..20].copy_from_slice(&bssid);
        frame.extend_from_slice(&hdr_bytes);
        frame
    }
}

// ============================================================================
// 802.11 Frame Parser
// ============================================================================
pub struct Ieee80211Parser;

#[derive(Debug, Clone)]
pub struct ParsedBeacon {
    pub bssid: [u8; 6],
    pub ssid: String,
    pub channel: u8,
    pub rssi: i8,
    pub capability: u16,
    pub has_rsn: bool,
    pub has_ht: bool,
    pub rsn_ie: Vec<u8>,
    pub wpa_ie: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ParsedProbeResp {
    pub bssid: [u8; 6],
    pub ssid: String,
    pub channel: u8,
    pub capability: u16,
    pub has_rsn: bool,
    pub has_ht: bool,
    pub rsn_ie: Vec<u8>,
    pub wpa_ie: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ParsedAuthResp {
    pub status: u16,
    pub auth_algo: u16,
    pub auth_seq: u16,
}

#[derive(Debug, Clone)]
pub struct ParsedAssocResp {
    pub status: u16,
    pub association_id: u16,
    pub capability: u16,
    pub supported_rates: Vec<u8>,
    /// Pairwise cipher selected by the AP in its RSN IE (0 if absent).
    pub pairwise: u32,
}

#[derive(Debug, Clone)]
pub struct ParsedDataFrame {
    pub bssid: [u8; 6],
    pub src_mac: [u8; 6],
    pub dst_mac: [u8; 6],
    pub payload: Vec<u8>,
}

impl Ieee80211Parser {
    fn parse_ies(data: &[u8]) -> (String, u8, bool, bool) {
        let mut ssid = String::new();
        let mut channel = 0u8;
        let mut has_rsn = false;
        let mut has_ht = false;
        let mut offset = 0;

        while offset + 2 <= data.len() {
            let ie_id = data[offset];
            let ie_len = data[offset + 1] as usize;
            offset += 2;

            if offset + ie_len > data.len() {
                break;
            }

            match ie_id {
                IE_SSID => {
                    if ie_len > 0 && ie_len <= 32 {
                        if let Ok(s) = core::str::from_utf8(&data[offset..offset + ie_len]) {
                            ssid = String::from(s);
                        }
                    }
                }
                IE_DS_PARAM => {
                    if ie_len >= 1 {
                        channel = data[offset];
                    }
                }
                IE_RSN => {
                    has_rsn = true;
                }
                IE_HT_CAPABILITIES => {
                    has_ht = true;
                }
                _ => {}
            }

            offset += ie_len;
        }

        (ssid, channel, has_rsn, has_ht)
    }

    /// Extract the raw RSN IE body and the WPA (vendor 221, OUI 00:50:F2,
    /// type 1) IE body from a beacon/probe-response IE blob. The assoc request
    /// uses the AP's RSN IE to negotiate matching ciphers.
    fn parse_security_ies(data: &[u8]) -> (Vec<u8>, Vec<u8>) {
        const WPA_OUI_TYPE: [u8; 4] = [0x00, 0x50, 0xF2, 0x01];
        let mut rsn = Vec::new();
        let mut wpa = Vec::new();
        let mut offset = 0;
        while offset + 2 <= data.len() {
            let ie_id = data[offset];
            let ie_len = data[offset + 1] as usize;
            offset += 2;
            if offset + ie_len > data.len() {
                break;
            }
            if ie_id == IE_RSN {
                rsn = data[offset..offset + ie_len].to_vec();
            } else if ie_id == IE_VENDOR_SPECIFIC
                && ie_len >= 4
                && data[offset..offset + 4] == WPA_OUI_TYPE
            {
                wpa = data[offset..offset + ie_len].to_vec();
            }
            offset += ie_len;
        }
        (rsn, wpa)
    }

    pub fn parse_beacon(frame: &[u8], rssi: i8) -> Option<ParsedBeacon> {
        if frame.len() < 24 + 12 {
            return None;
        }

        let hdr = unsafe { &*(frame.as_ptr() as *const Ieee80211Header) };
        let subtype = hdr.frame_subtype();
        if subtype != FC_SUBTYPE_BEACON {
            return None;
        }

        let bssid = hdr.addr3;
        let ie_data = &frame[24 + 12..]; // Skip fixed fields (timestamp 8, beacon interval 2, capability 2)

        let (ssid, channel, has_rsn, has_ht) = Self::parse_ies(ie_data);
        let (rsn_ie, wpa_ie) = Self::parse_security_ies(ie_data);
        let capability = u16::from_le_bytes([frame[24 + 10], frame[24 + 11]]);

        Some(ParsedBeacon {
            bssid,
            ssid,
            channel: if channel == 0 { 1 } else { channel },
            rssi,
            capability,
            has_rsn,
            has_ht,
            rsn_ie,
            wpa_ie,
        })
    }

    pub fn parse_probe_response(frame: &[u8]) -> Option<ParsedProbeResp> {
        if frame.len() < 24 + 12 {
            return None;
        }

        let hdr = unsafe { &*(frame.as_ptr() as *const Ieee80211Header) };
        let subtype = hdr.frame_subtype();
        if subtype != FC_SUBTYPE_PROBE_RESP {
            return None;
        }

        let bssid = hdr.addr3;
        let ie_data = &frame[24 + 12..];
        let (ssid, channel, has_rsn, has_ht) = Self::parse_ies(ie_data);
        let (rsn_ie, wpa_ie) = Self::parse_security_ies(ie_data);
        let capability = u16::from_le_bytes([frame[24 + 10], frame[24 + 11]]);

        Some(ParsedProbeResp {
            bssid,
            ssid,
            channel: if channel == 0 { 1 } else { channel },
            capability,
            has_rsn,
            has_ht,
            rsn_ie,
            wpa_ie,
        })
    }

    pub fn parse_auth_response(frame: &[u8]) -> Option<ParsedAuthResp> {
        if frame.len() < 24 + 6 {
            return None;
        }

        let hdr = unsafe { &*(frame.as_ptr() as *const Ieee80211Header) };
        if hdr.frame_subtype() != FC_SUBTYPE_AUTH {
            return None;
        }

        let algo = u16::from_le_bytes([frame[24], frame[24 + 1]]);
        let seq = u16::from_le_bytes([frame[24 + 2], frame[24 + 3]]);
        let status = u16::from_le_bytes([frame[24 + 4], frame[24 + 5]]);

        Some(ParsedAuthResp {
            status,
            auth_algo: algo,
            auth_seq: seq,
        })
    }

    pub fn parse_assoc_response(frame: &[u8]) -> Option<ParsedAssocResp> {
        if frame.len() < 24 + 6 {
            return None;
        }

        let hdr = unsafe { &*(frame.as_ptr() as *const Ieee80211Header) };
        let subtype = hdr.frame_subtype();
        if subtype != FC_SUBTYPE_ASSOC_RESP {
            return None;
        }

        let capability = u16::from_le_bytes([frame[24], frame[24 + 1]]);
        let status = u16::from_le_bytes([frame[24 + 2], frame[24 + 3]]);
        let aid = u16::from_le_bytes([frame[24 + 4], frame[24 + 5]]);

        let mut rates = Vec::new();
        let mut pairwise = 0u32;
        if frame.len() > 24 + 6 {
            let ie_data = &frame[24 + 6..];
            let mut offset = 0;
            while offset + 2 <= ie_data.len() {
                let ie_id = ie_data[offset];
                let ie_len = ie_data[offset + 1] as usize;
                offset += 2;
                if offset + ie_len > ie_data.len() {
                    break;
                }
                if ie_id == IE_SUPPORTED_RATES {
                    rates.extend_from_slice(&ie_data[offset..offset + ie_len]);
                } else if ie_id == 48 && ie_len >= 14 {
                    // RSN IE (AssocResp): the AP puts its SELECTED pairwise
                    // cipher here. Reading it avoids assuming CCMP when a
                    // mixed WPA/WPA2 AP actually chose TKIP.
                    let d = &ie_data[offset..offset + ie_len];
                    let count = u16::from_le_bytes([d[8], d[9]]) as usize;
                    if count >= 1 {
                        pairwise = u32::from_be_bytes([d[10], d[11], d[12], d[13]]);
                    }
                }
                offset += ie_len;
            }
        }

        Some(ParsedAssocResp {
            status,
            association_id: aid & 0x3FFF,
            capability,
            supported_rates: rates,
            pairwise,
        })
    }

    pub fn parse_data_frame(frame: &[u8]) -> Option<ParsedDataFrame> {
        if frame.len() < 24 {
            return None;
        }

        let hdr = unsafe { &*(frame.as_ptr() as *const Ieee80211Header) };
        let fc = u16::from_le(hdr.frame_control);
        let frame_type = fc & 0x000C;
        let subtype = fc & 0x00F0;

        // Check if it's a data frame (type 2 = 0x08) with QoS (subtype 8 = 0x0080)
        if frame_type != FC_TYPE_DATA {
            return None;
        }

        let to_ds = (fc & FC_TO_DS) != 0;
        let from_ds = (fc & FC_FROM_DS) != 0;

        let (bssid, src, dst) = if to_ds && !from_ds {
            // STA -> AP: addr1=RA=BSSID, addr2=TA=SA, addr3=DA
            (hdr.addr1, hdr.addr2, hdr.addr3)
        } else if !to_ds && from_ds {
            // AP -> STA: addr1=DA, addr2=BSSID, addr3=SA
            (hdr.addr2, hdr.addr3, hdr.addr1)
        } else if !to_ds && !from_ds {
            // IBSS / direct data: addr1=DA, addr2=SA, addr3=BSSID
            (hdr.addr3, hdr.addr2, hdr.addr1)
        } else {
            return None;
        };

        // QoS data frame has extra 2 bytes (QoS control)
        let qos_len = if subtype == 0x0080 { 2 } else { 0 };
        // Protected frames (CCMP) carry an 8-byte CCMP header (PN + key id)
        // between the MAC header and the LLC/SNAP payload. The hardware
        // decrypts the payload but leaves that header in place, so skip it.
        let protected = (fc & 0x4000) != 0;
        let header_len = 24 + qos_len + if protected { 8 } else { 0 };
        if frame.len() <= header_len {
            return None;
        }

        // Check for LLC/SNAP header (AA AA 03 00 00 00)
        let payload = &frame[header_len..];
        if payload.len() >= 8
            && payload[0] == 0xAA
            && payload[1] == 0xAA
            && payload[2] == 0x03
            && payload[3] == 0x00
            && payload[4] == 0x00
            && payload[5] == 0x00
        {
            // LLC/SNAP header present, extract ethertype and payload
            let ethertype = u16::from_be_bytes([payload[6], payload[7]]);
            let data = &payload[8..];
            // Reconstruct as Ethernet frame for the stack
            let mut eth_frame = Vec::with_capacity(14 + data.len());
            eth_frame.extend_from_slice(&dst); // dst_mac
            eth_frame.extend_from_slice(&src); // src_mac
            eth_frame.extend_from_slice(&ethertype.to_be_bytes());
            eth_frame.extend_from_slice(data);

            Some(ParsedDataFrame {
                bssid,
                src_mac: src,
                dst_mac: dst,
                payload: eth_frame,
            })
        } else {
            // No LLC/SNAP, pass through raw
            Some(ParsedDataFrame {
                bssid,
                src_mac: src,
                dst_mac: dst,
                payload: payload.to_vec(),
            })
        }
    }

    /// Build a QoS data frame with LLC/SNAP header for Ethernet-over-802.11
    /// (STA -> AP, ToDS=1, FromDS=0).
    pub fn build_data_frame(
        bssid: [u8; 6],
        src_mac: [u8; 6],
        dst_mac: [u8; 6],
        ethertype: u16,
        payload: &[u8],
    ) -> Vec<u8> {
        let mut frame = Vec::with_capacity(34 + payload.len());

        // Plain DATA subtype (0x0008) + ToDS: FC = 0x0108.
        // NOTE: we never negotiate WMM/WME in the association request, so we
        // are a non-QoS STA and MUST send non-QoS Data frames. Sending QoS Data
        // made strict APs (e.g. mobile hotspots with WMM) drop every frame —
        // including the EAPOL 2/4 — so the 4-way handshake never completed.
        let fc: u16 = FC_TYPE_DATA | FC_TO_DS;
        let mut hdr = [0u8; 24];
        hdr[0..2].copy_from_slice(&fc.to_le_bytes()); // Frame Control
        hdr[2..4].copy_from_slice(&0u16.to_le_bytes()); // Duration
        hdr[4..10].copy_from_slice(&bssid); // addr1 = RA = BSSID
        hdr[10..16].copy_from_slice(&src_mac); // addr2 = TA = us
        hdr[16..22].copy_from_slice(&dst_mac); // addr3 = DA
        // Unique incrementing sequence number. Every data frame used to carry
        // seq=0, so after the AP accepted msg2 (seq 0) it dropped every msg4
        // (also seq 0) as a *duplicate* of the same sequence: the AP kept
        // retransmitting msg3 and finally deauthenticated us. This is shared
        // with build_qos_null so keepalives cannot collide either.
        hdr[22..24].copy_from_slice(&next_seq_ctrl().to_le_bytes()); // Sequence Control
        frame.extend_from_slice(&hdr);

        // LLC/SNAP header
        frame.extend_from_slice(&[0xAA, 0xAA, 0x03, 0x00, 0x00, 0x00]);
        frame.extend_from_slice(&ethertype.to_be_bytes());

        // Payload
        frame.extend_from_slice(payload);

        frame
    }

    /// Build a Null data frame (subtype 0x0040) used to keep an association
    /// alive (STA -> AP, ToDS=1). Non-QoS, matching our non-WMM association.
    pub fn build_qos_null(bssid: [u8; 6], tx_addr: [u8; 6]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(24);
        let fc: u16 = FC_TYPE_DATA | FC_TO_DS | 0x0040; // Null (no data)
        let mut hdr = [0u8; 24];
        hdr[0..2].copy_from_slice(&fc.to_le_bytes());
        hdr[4..10].copy_from_slice(&bssid); // addr1 = RA = BSSID
        hdr[10..16].copy_from_slice(&tx_addr); // addr2 = TA = us
        hdr[16..22].copy_from_slice(&bssid); // addr3 = BSSID
        hdr[22..24].copy_from_slice(&next_seq_ctrl().to_le_bytes());
        frame.extend_from_slice(&hdr);
        frame
    }
}
