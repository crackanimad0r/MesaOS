//! WiFi Scanning Module
//!
//! Implements active WiFi scanning by sending probe requests on each channel
//! and collecting probe responses / beacons.

use alloc::vec::Vec;
use alloc::string::String;
use spin::Mutex;
use core::sync::atomic::{AtomicBool, Ordering};

use super::frame::*;

// ============================================================================
// 2.4 GHz Channels
// ============================================================================
pub const CHANNELS_2G: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];

/// Time to dwell on each channel during scan (ms)
const CHANNEL_DWELL_MS: u64 = 80;

/// Maximum scan time per channel for probe response wait (ms)
const PROBE_RESPONSE_TIMEOUT_MS: u64 = 100;

// ============================================================================
// Scan Results
// ============================================================================
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub bssid: [u8; 6],
    pub ssid: String,
    pub channel: u8,
    pub rssi: i8,
    pub capability: u16,
    pub has_rsn: bool,
    pub has_ht: bool,
    /// Raw RSN IE body (without ID/len) advertised by the AP. Used to build a
    /// matching RSN IE in the assoc request (WPA/WPA2 mixed group ciphers).
    pub rsn_ie: Vec<u8>,
    /// Raw WPA vendor IE body (221/00:50:F2/01), if present.
    pub wpa_ie: Vec<u8>,
}

pub static SCAN_RESULTS: Mutex<Vec<ScanResult>> = Mutex::new(Vec::new());
pub static SCAN_COMPLETE: Mutex<bool> = Mutex::new(false);
pub static SCANNING: Mutex<bool> = Mutex::new(false);

/// When false (normal `wifi scan`) only one concise line is printed per newly
/// discovered BSSID. When true (`wifi scanner`) every RX frame / PHY counter is
/// dumped verbosely.
static SCAN_VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set_verbose(v: bool) {
    SCAN_VERBOSE.store(v, Ordering::Relaxed);
}

pub fn is_verbose() -> bool {
    SCAN_VERBOSE.load(Ordering::Relaxed)
}

/// Add a beacon/probe response to scan results. Returns true if the BSSID was
/// not seen before (i.e. genuinely new), false if it was already present.
pub fn add_scan_result(result: ScanResult) -> bool {
    let mut results = SCAN_RESULTS.lock();
    // Deduplicate by BSSID: never count the same AP twice.
    if results.iter().any(|r| r.bssid == result.bssid) {
        return false;
    }
    let enc = if result.has_rsn { "WPA2" } else { "Open " };
    if is_verbose() {
        crate::mesa_println!("Beacon: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}  \"{}\"  ch={} RSSI={}dBm [{}]",
            result.bssid[0], result.bssid[1], result.bssid[2],
            result.bssid[3], result.bssid[4], result.bssid[5],
            result.ssid, result.channel, result.rssi, enc);
    }
    results.push(result);
    true
}

/// Number of unique networks found so far.
pub fn result_count() -> usize {
    SCAN_RESULTS.lock().len()
}

/// Clear all scan results
pub fn clear_results() {
    SCAN_RESULTS.lock().clear();
    *SCAN_COMPLETE.lock() = false;
}

/// Get current scan results
pub fn get_results() -> Vec<ScanResult> {
    SCAN_RESULTS.lock().clone()
}

/// Check if scan is complete
pub fn is_complete() -> bool {
    *SCAN_COMPLETE.lock()
}

pub fn set_complete(val: bool) {
    *SCAN_COMPLETE.lock() = val;
}

pub fn is_scanning() -> bool {
    *SCANNING.lock()
}

pub fn set_scanning(val: bool) {
    *SCANNING.lock() = val;
}

/// Build a probe request for a specific channel
pub fn build_probe_request(tx_addr: [u8; 6], ssid: &str) -> Vec<u8> {
    // Broadcast BSSID for probe request
    let broadcast = [0xFF; 6];
    IeBuilder::build_probe_request(broadcast, ssid, tx_addr)
}

/// Parse a received management frame and add to scan results if applicable
pub fn process_scan_frame(frame: &[u8], rssi: i8) {
    if frame.len() < 24 {
        return;
    }

    // Only collect/print beacon & probe-response results while a scan is
    // actually running. Otherwise the periodically received beacons of the
    // associated AP would keep printing "Beacon:" lines with no command issued.
    if !is_scanning() {
        return;
    }

    let hdr = unsafe { &*(frame.as_ptr() as *const Ieee80211Header) };
    let subtype = hdr.frame_subtype();

    match subtype {
        FC_SUBTYPE_BEACON => {
            if let Some(beacon) = Ieee80211Parser::parse_beacon(frame, rssi) {
                if !beacon.ssid.is_empty() && beacon.ssid != "\0" {
                    add_scan_result(ScanResult {
                        bssid: beacon.bssid,
                        ssid: beacon.ssid,
                        channel: beacon.channel,
                        rssi: beacon.rssi,
                        capability: beacon.capability,
                        has_rsn: beacon.has_rsn,
                        has_ht: beacon.has_ht,
                        rsn_ie: beacon.rsn_ie,
                        wpa_ie: beacon.wpa_ie,
                    });
                }
            }
        }
        FC_SUBTYPE_PROBE_RESP => {
            if let Some(resp) = Ieee80211Parser::parse_probe_response(frame) {
                if !resp.ssid.is_empty() && resp.ssid != "\0" {
                    add_scan_result(ScanResult {
                        bssid: resp.bssid,
                        ssid: resp.ssid,
                        channel: resp.channel,
                        rssi: -50, // Default RSSI for probe responses
                        capability: resp.capability,
                        has_rsn: resp.has_rsn,
                        has_ht: resp.has_ht,
                        rsn_ie: resp.rsn_ie,
                        wpa_ie: resp.wpa_ie,
                    });
                }
            }
        }
        _ => {}
    }
}
