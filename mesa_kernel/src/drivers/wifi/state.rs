//! WiFi State Machine
//!
//! Manages the connection state: Disconnected -> Scanning -> Authenticating -> Associated -> Connected

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

use super::frame::*;
use super::scan::ScanResult;

// ============================================================================
// WiFi States
// ============================================================================
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WifiState {
    Disconnected,
    Scanning,
    Authenticating,
    Associating,
    Connected,
    Error,
}

impl WifiState {
    pub fn name(&self) -> &'static str {
        match self {
            WifiState::Disconnected => "Disconnected",
            WifiState::Scanning => "Scanning",
            WifiState::Authenticating => "Authenticating",
            WifiState::Associating => "Associating",
            WifiState::Connected => "Connected",
            WifiState::Error => "Error",
        }
    }
}

// ============================================================================
// WiFi Connection State
// ============================================================================
pub struct WifiStateInfo {
    pub state: WifiState,
    pub ssid: String,
    pub bssid: [u8; 6],
    pub channel: u8,
    pub association_id: u16,
    pub has_rsn: bool,
    pub tx_addr: [u8; 6],
    /// Raw RSN IE body advertised by the AP (empty for open networks).
    pub rsn_ie: Vec<u8>,
}

impl WifiStateInfo {
    pub const fn new() -> Self {
        Self {
            state: WifiState::Disconnected,
            ssid: String::new(),
            bssid: [0; 6],
            channel: 0,
            association_id: 0,
            has_rsn: false,
            tx_addr: [0; 6],
            rsn_ie: Vec::new(),
        }
    }
}

pub static WIFI_STATE: Mutex<WifiStateInfo> = Mutex::new(WifiStateInfo::new());

pub fn get_state() -> WifiState {
    WIFI_STATE.lock().state
}

pub fn set_state(state: WifiState) {
    WIFI_STATE.lock().state = state;
}

pub fn get_ssid() -> String {
    WIFI_STATE.lock().ssid.clone()
}

pub fn get_bssid() -> [u8; 6] {
    WIFI_STATE.lock().bssid
}

pub fn get_channel() -> u8 {
    WIFI_STATE.lock().channel
}

pub fn get_tx_addr() -> [u8; 6] {
    WIFI_STATE.lock().tx_addr
}

pub fn set_tx_addr(addr: [u8; 6]) {
    WIFI_STATE.lock().tx_addr = addr;
}

/// Build the authentication frame to send to the AP
pub fn build_auth_frame(bssid: [u8; 6], tx_addr: [u8; 6]) -> Vec<u8> {
    IeBuilder::build_auth_open(1, bssid, tx_addr, bssid)
}

/// RSN IE that must be used BOTH in the association request and in the
/// EAPOL msg 2/4 key data. The AP stores the RSN IE from the assoc and
/// rejects a msg 2/4 whose RSN IE differs (e.g. a hardcoded CCMP group when
/// the AP negotiated a TKIP group) by retransmitting msg 1/4 and finally
/// disassociating us.
pub fn get_assoc_rsn_ie() -> Vec<u8> {
    let state = WIFI_STATE.lock();
    if !state.has_rsn {
        return Vec::new();
    }
    if state.rsn_ie.len() >= 8 {
        IeBuilder::add_rsn_from_ap(&state.rsn_ie)
    } else {
        IeBuilder::add_rsn_wpa2_psk()
    }
}

/// True when the AP's RSN IE advertises a TKIP group cipher (WPA/WPA2 mixed
/// mode). The GTK must then be written to the CAM as TKIP so the hardware can
/// decrypt broadcast/multicast (e.g. the DHCP OFFER).
pub fn group_cipher_is_tkip() -> bool {
    let state = WIFI_STATE.lock();
    state.has_rsn
        && state.rsn_ie.len() >= 6
        && state.rsn_ie[2..6] == [0x00, 0x0F, 0xAC, 0x02]
}

/// Build the association request frame
pub fn build_assoc_request(bssid: [u8; 6], tx_addr: [u8; 6], ssid: &str) -> Vec<u8> {
    // Include the RSN IE when the target network is secured, mirroring the
    // suites the AP advertises (WPA/WPA2 mixed routers use a TKIP group).
    let (include_rsn, ap_rsn) = {
        let state = WIFI_STATE.lock();
        (state.has_rsn, state.rsn_ie.clone())
    };
    IeBuilder::build_assoc_req(bssid, tx_addr, ssid, 1, include_rsn, &ap_rsn)
}

/// Build deauthentication frame
pub fn build_deauth_frame(bssid: [u8; 6], tx_addr: [u8; 6]) -> Vec<u8> {
    IeBuilder::build_deauth(bssid, tx_addr, bssid, REASON_LEAVING)
}

/// Process an authentication response
/// Returns `None` when the frame is not an Open System auth response for us
/// (it must be ignored, not treated as a rejection), `Some(true)` on success
/// and `Some(false)` on an explicit rejection (status != 0).
pub fn process_auth_response(frame: &[u8]) -> Option<bool> {
    match Ieee80211Parser::parse_auth_response(frame) {
        Some(resp) => {
            // An Open System auth RESPONSE is algo=0, seq=2. Auth requests
            // (seq=1, e.g. from other stations, visible because RCR AAP=1)
            // must not drive our state machine.
            if resp.auth_algo != AUTH_ALGO_OPEN || resp.auth_seq != 2 {
                crate::serial_println!("[WiFi] Ignoring auth frame algo={} seq={}",
                    resp.auth_algo, resp.auth_seq);
                return None;
            }
            if resp.status == STATUS_SUCCESS {
                crate::serial_println!("[WiFi] Auth OK (algo={}, seq={})", resp.auth_algo, resp.auth_seq);
                Some(true)
            } else {
                crate::serial_println!("[WiFi] Auth FAILED: status={}", resp.status);
                Some(false)
            }
        }
        None => {
            crate::serial_println!("[WiFi] Failed to parse auth response");
            None
        }
    }
}

/// Process an association response
pub fn process_assoc_response(frame: &[u8]) -> Option<u16> {
    match Ieee80211Parser::parse_assoc_response(frame) {
        Some(resp) => {
            if resp.status == STATUS_SUCCESS {
                let pw = resp.pairwise;
                let pw_name = if pw == 0x000F_AC04 {
                    "CCMP"
                } else if pw == 0x000F_AC02 {
                    "TKIP"
                } else if pw == 0 {
                    "?"
                } else {
                    "otro"
                };
                crate::mesa_println!(
                    "[WiFi] Assoc OK (AID={}) cipher pairwise={} ({:#010x})",
                    resp.association_id, pw_name, pw
                );
                Some(resp.association_id)
            } else {
                crate::serial_println!("[WiFi] Assoc FAILED: status={}", resp.status);
                None
            }
        }
        None => {
            crate::serial_println!("[WiFi] Failed to parse assoc response");
            None
        }
    }
}

/// Start the connection process to a specific AP
pub fn start_connect(result: &ScanResult, tx_addr: [u8; 6]) {
    let mut state = WIFI_STATE.lock();
    state.state = WifiState::Authenticating;
    state.ssid = result.ssid.clone();
    state.bssid = result.bssid;
    state.channel = result.channel;
    state.has_rsn = result.has_rsn;
    state.tx_addr = tx_addr;
    state.rsn_ie = result.rsn_ie.clone();

    crate::serial_println!(
        "[WiFi] Connecting to '{}' ch={} BSSID={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        state.ssid, state.channel,
        state.bssid[0], state.bssid[1], state.bssid[2],
        state.bssid[3], state.bssid[4], state.bssid[5]
    );
}

/// Handle a received frame in the context of the connection state machine
pub fn handle_frame(frame: &[u8]) {
    if frame.len() < 24 {
        return;
    }

    let hdr = unsafe { &*(frame.as_ptr() as *const Ieee80211Header) };
    let subtype = hdr.frame_subtype();
    let mut state = WIFI_STATE.lock();

    // A frame only belongs to our connection when addr1 is our MAC. The RX
    // filter accepts every unicast frame (RCR AAP=1), so without this check a
    // neighbour's auth response or deauth drove our state machine (which made
    // us "authenticate" with a stranger and then abort with a bogus reason 9).
    let our = state.tx_addr;
    let addr1 = &frame[4..10];
    let to_us = addr1 == &our[..];
    let ap_response = to_us;

    match state.state {
        WifiState::Authenticating => {
            if subtype == FC_SUBTYPE_AUTH && ap_response {
                match process_auth_response(frame) {
                    Some(true) => {
                        state.state = WifiState::Associating;
                        drop(state);
                        // Will be picked up by the polling loop to send assoc
                    }
                    Some(false) => state.state = WifiState::Disconnected,
                    None => {} // not our response: keep waiting
                }
            }
        }
        WifiState::Associating => {
            if subtype == FC_SUBTYPE_ASSOC_RESP && ap_response {
                if let Some(aid) = process_assoc_response(frame) {
                    state.association_id = aid;
                    state.state = WifiState::Connected;
                    crate::serial_println!(
                        "[WiFi] Connected to '{}' (AID={})",
                        state.ssid, aid
                    );
                } else {
                    state.state = WifiState::Disconnected;
                }
            } else if (subtype == FC_SUBTYPE_DEAUTH || subtype == FC_SUBTYPE_DISASSOC) && to_us {
                let reason = if frame.len() >= 26 { u16::from_le_bytes([frame[24], frame[25]]) } else { 0 };
                crate::mesa_println!("[WiFi] Deauth/Disassoc during association (reason={})", reason);
                // 2 = previous authentication no longer valid, 6 = Class 2
                // frame from a non-authenticated STA, 7 = Class 3 frame from a
                // non-associated STA, 9 = STA not authenticated. All of them
                // mean "the AP lost/changed our state", not "go away": real
                // routers (and buggy ISP firmware) send 2 on the first assoc
                // after a restart/stale entry, and a fresh auth clears it.
                // Only give up for the remaining reasons.
                if matches!(reason, 2 | 6 | 7 | 9) {
                    state.state = WifiState::Authenticating;
                } else {
                    state.state = WifiState::Disconnected;
                }
            }
        }
        WifiState::Connected => {
            if subtype == FC_SUBTYPE_DEAUTH && to_us {
                let reason = if frame.len() >= 26 { u16::from_le_bytes([frame[24], frame[25]]) } else { 0 };
                crate::mesa_println!("[WiFi] DESCONECTADO: deauth del AP (reason={})", reason);
                state.state = WifiState::Disconnected;
            } else if subtype == FC_SUBTYPE_DISASSOC && to_us {
                let reason = if frame.len() >= 26 { u16::from_le_bytes([frame[24], frame[25]]) } else { 0 };
                crate::mesa_println!("[WiFi] DESCONECTADO: disassoc del AP (reason={})", reason);
                state.state = WifiState::Disconnected;
            }
        }
        _ => {}
    }
}

/// Get whether the network uses RSN (WPA2/WPA3)
pub fn get_has_rsn() -> bool {
    WIFI_STATE.lock().has_rsn
}

/// Get the pairwise (unicast) PTK key if available
pub fn get_pairwise_key() -> Option<alloc::vec::Vec<u8>> {
    // In a full implementation, this would retrieve the PTK from the 4-way handshake
    // For now, return None (driver will handle key derivation)
    None
}

/// Get the group (broadcast/multicast) GTK key if available
pub fn get_group_key() -> Option<alloc::vec::Vec<u8>> {
    // In a full implementation, this would retrieve the GTK from the 4-way handshake
    // For now, return None
    None
}

// ============================================================================
// Last-connected AP cache (avoids re-scanning automatically on reconnect)
// ============================================================================

/// Cache of the last SSID -> (BSSID, channel) mapping, so `wifi_connect` can
/// skip the full channel scan when reconnecting to the same AP (prevents the
/// system from "scanning by itself" without user action).
static mut LAST_SSID: alloc::string::String = alloc::string::String::new();
static mut LAST_BSSID: [u8; 6] = [0u8; 6];
static mut LAST_CHANNEL: u8 = 0;
static mut LAST_HAS_RSN: bool = false;

/// Store the BSSID and channel associated with an SSID after a successful
/// connection. Also stores whether the AP is secured (RSN), because a
/// reconnect to a secured AP must include the RSN IE in the association.
pub fn cache_last_bssid(ssid: &str, bssid: [u8; 6], channel: u8, has_rsn: bool) {
    unsafe {
        LAST_SSID = alloc::string::String::from(ssid);
        LAST_BSSID = bssid;
        LAST_CHANNEL = channel;
        LAST_HAS_RSN = has_rsn;
    }
}

/// Retrieve the cached BSSID, channel and secured flag for an SSID, if we
/// have connected to it before. Returns `None` if no cached entry matches.
pub fn get_cached_bssid(ssid: &str) -> Option<([u8; 6], u8, bool)> {
    unsafe {
        if LAST_SSID == ssid && LAST_BSSID != [0u8; 6] {
            Some((LAST_BSSID, LAST_CHANNEL, LAST_HAS_RSN))
        } else {
            None
        }
    }
}

/// Clear the cached BSSID (e.g. on explicit disconnect).
pub fn clear_cached_bssid() {
    unsafe {
        LAST_SSID = alloc::string::String::new();
        LAST_BSSID = [0u8; 6];
        LAST_CHANNEL = 0;
        LAST_HAS_RSN = false;
    }
}
