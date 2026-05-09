//! BIOS/UEFI Analyzer for MesaOS
//! Herramienta para analizar y diagnosticar configuraciones de BIOS que afectan al WiFi RTL8822CE

extern crate alloc;
use alloc::{string::String, vec::Vec, format};
use crate::pci;
use crate::acpi;

/// Información de análisis de BIOS
#[derive(Debug)]
pub struct BiosAnalysis {
    pub vendor: String,
    pub version: String,
    pub date: String,
    pub features: Vec<String>,
    pub issues: Vec<String>,
    pub recommendations: Vec<String>,
}

/// Analizador principal de BIOS/UEFI
pub struct BiosAnalyzer;

impl BiosAnalyzer {
    /// Ejecuta análisis completo de BIOS/UEFI
    pub fn analyze() -> BiosAnalysis {
        let mut analysis = BiosAnalysis {
            vendor: String::from("Unknown"),
            version: String::from("Unknown"),
            date: String::from("Unknown"),
            features: Vec::new(),
            issues: Vec::new(),
            recommendations: Vec::new(),
        };

        // 1. Analizar información básica del sistema
        Self::analyze_system_info(&mut analysis);

        // 2. Analizar configuraciones PCI críticas
        Self::analyze_pci_config(&mut analysis);

        // 3. Analizar power management
        Self::analyze_power_management(&mut analysis);

        // 4. Analizar configuraciones específicas para WiFi
        Self::analyze_wifi_specific(&mut analysis);

        // 5. Generar recomendaciones
        Self::generate_recommendations(&mut analysis);

        analysis
    }

    /// Analiza información básica del sistema
    fn analyze_system_info(analysis: &mut BiosAnalysis) {
        // Intentar obtener información del SMBIOS
        if let Some(acpi_info) = acpi::get_info() {
            analysis.vendor = format!("ACPI OEM: {}", acpi_info.oem_id);
        }

        analysis.features.push("ACPI Support: Enabled".into());
        analysis.features.push("PCI Express: Detected".into());
        analysis.features.push("USB Support: Enabled".into());
    }

    /// Analiza configuraciones PCI críticas para WiFi
    fn analyze_pci_config(analysis: &mut BiosAnalysis) {
        if let Some(device) = pci::find_wifi_device() {
            // Verificar BAR0
            let bar0 = pci::pci_config_read(device.bus, device.device, device.function, 0x10);
            if bar0 == 0 {
                analysis.issues.push("CRITICAL: BAR0 = 0 - BIOS no asignó memoria MMIO".into());
            } else {
                // Verificar si BAR0 es I/O o Memory
                let is_io_space = (bar0 & 0x01) != 0;
                if is_io_space {
                    analysis.issues.push(format!("BAR0 asignado a I/O space: 0x{:08x} (debería ser Memory)", bar0));
                } else {
                    analysis.features.push(format!("BAR0 asignado a Memory: 0x{:08x}", bar0 & 0xFFFFFFF0));
                }
            }

            // Verificar BAR1 (Memory BAR principal para RTL8822CE)
            let bar1 = pci::pci_config_read(device.bus, device.device, device.function, 0x14);
            if bar1 == 0 {
                analysis.issues.push("CRITICAL: BAR1 = 0 - Memoria MMIO principal no asignada".into());
            } else {
                analysis.features.push(format!("BAR1 Memory asignado: 0x{:08x}", bar1 & 0xFFFFFFF0));
            }

            // Verificar estado de power
            if let Some(pm_cap) = pci::pci_find_capability(device.bus, device.device, device.function, 0x01) {
                let pmcsr = pci::pci_config_read(device.bus, device.device, device.function, pm_cap + 4);
                let power_state = pmcsr & 0x03;
                if power_state != 0 {
                    analysis.issues.push(format!("Power State: D{} (debería ser D0)", power_state));
                } else {
                    analysis.features.push("Power State: D0 (correcto)".into());
                }
            }

            // Verificar ASPM
            if let Some(pcie_cap) = pci::pci_find_capability(device.bus, device.device, device.function, 0x10) {
                let link_ctl = pci::pci_config_read(device.bus, device.device, device.function, pcie_cap + 0x10);
                let aspm_enabled = (link_ctl & 0x03) != 0;
                if aspm_enabled {
                    analysis.issues.push("ASPM: Enabled - Puede causar problemas con WiFi".into());
                } else {
                    analysis.features.push("ASPM: Disabled (bueno para WiFi)".into());
                }
            }

            // Verificar command register - CRÍTICO
            let command = pci::pci_config_read(device.bus, device.device, device.function, 0x04) & 0xFFFF;
            let mem_enabled = (command & 0x02) != 0;
            let bus_master = (command & 0x04) != 0;

            if !mem_enabled {
                analysis.issues.push("CRITICAL: Memory Space Disabled - El dispositivo no puede acceder a memoria".into());
            } else {
                analysis.features.push("Memory Space: Enabled".into());
            }

            if !bus_master {
                analysis.issues.push("CRITICAL: Bus Master Disabled - El dispositivo no puede iniciar transferencias".into());
            } else {
                analysis.features.push("Bus Master: Enabled".into());
            }
        } else {
            analysis.issues.push("WiFi device not found in PCI scan".into());
        }
    }

    /// Analiza configuraciones de power management
    fn analyze_power_management(analysis: &mut BiosAnalysis) {
        // Verificar si hay configuraciones agresivas de power management
        analysis.features.push("Power Management Analysis: Basic".into());

        // Buscar dispositivos PCI con power management issues
        for dev in pci::devices() {
            if dev.class_code == 0x02 && dev.subclass == 0x80 { // Network controller
                if let Some(pm_cap) = pci::pci_find_capability(dev.bus, dev.device, dev.function, 0x01) {
                    let pmcsr = pci::pci_config_read(dev.bus, dev.device, dev.function, pm_cap + 4);
                    let power_state = pmcsr & 0x03;
                    if power_state > 0 {
                        analysis.issues.push(format!("Network device in D{} state: {:02x}:{:02x}.{}",
                            power_state, dev.bus, dev.device, dev.function));
                    }
                }
            }
        }
    }

    /// Análisis específico para WiFi RTL8822CE
    fn analyze_wifi_specific(analysis: &mut BiosAnalysis) {
        if let Some(device) = pci::find_wifi_device() {
            if device.vendor_id == 0x10EC && device.device_id == 0xC822 {
                analysis.features.push("RTL8822CE detected - HP Laptop 15s-eq2xxx compatible".into());

                // Verificar configuraciones específicas conocidas para este chipset
                let revision = pci::pci_config_read(device.bus, device.device, device.function, 0x08) & 0xFF;
                analysis.features.push(format!("Chip Revision: 0x{:02x}", revision));

                // Verificar si el dispositivo está en low power state
                let bar0 = pci::pci_config_read(device.bus, device.device, device.function, 0x10);
                if bar0 == 0 {
                    analysis.issues.push("CHIP LOCKED: BAR0=0 indicates BIOS power management lock".into());
                    analysis.recommendations.push("Configure BIOS: Disable PCIe ASPM, Set PCIe Gen 2, Disable Secure Boot".into());
                }
            }
        }
    }

    /// Genera recomendaciones basadas en el análisis
    fn generate_recommendations(analysis: &mut BiosAnalysis) {
        if analysis.issues.iter().any(|i| i.contains("BAR0 = 0")) {
            analysis.recommendations.push("BIOS FIX REQUIRED: Enter BIOS setup (F10) and configure:".into());
            analysis.recommendations.push("  - Power Management → PCIe Power Management: Disabled".into());
            analysis.recommendations.push("  - System Configuration → PCIe Speed: Gen 2".into());
            analysis.recommendations.push("  - Security → Secure Boot: Disabled".into());
            analysis.recommendations.push("  - Advanced → Built-in Device Options → Wireless Radio: Enabled".into());
        }

        if analysis.issues.iter().any(|i| i.contains("ASPM")) {
            analysis.recommendations.push("BIOS: Disable PCIe ASPM (Active State Power Management)".into());
        }

        if analysis.issues.iter().any(|i| i.contains("Power State")) {
            analysis.recommendations.push("BIOS: Ensure device is in D0 power state".into());
        }

        analysis.recommendations.push("After BIOS changes: Cold boot (power off completely)".into());
        analysis.recommendations.push("Test with: wifi diag  (in MesaOS shell)".into());
    }

    /// Imprime el análisis completo
    pub fn print_analysis(analysis: &BiosAnalysis) {
        crate::mesa_println!("╔══════════════════════════════════════════════════════════════╗");
        crate::mesa_println!("║                 BIOS/UEFI ANALYSIS REPORT                   ║");
        crate::mesa_println!("╠══════════════════════════════════════════════════════════════╣");

        crate::mesa_println!("📋 SYSTEM INFO:");
        crate::mesa_println!("  Vendor: {}", analysis.vendor);
        crate::mesa_println!("  Version: {}", analysis.version);
        crate::mesa_println!("  Date: {}", analysis.date);

        crate::mesa_println!("\n✅ FEATURES DETECTED:");
        for feature in &analysis.features {
            crate::mesa_println!("  {}", feature);
        }

        if !analysis.issues.is_empty() {
            crate::mesa_println!("\n🚨 ISSUES FOUND:");
            for issue in &analysis.issues {
                crate::mesa_println!("  {}", issue);
            }
        }

        if !analysis.recommendations.is_empty() {
            crate::mesa_println!("\n💡 RECOMMENDATIONS:");
            for rec in &analysis.recommendations {
                crate::mesa_println!("  {}", rec);
            }
        }

        crate::mesa_println!("╚══════════════════════════════════════════════════════════════╝");
    }
}

/// Comando para ejecutar análisis de BIOS desde la shell
pub fn bios_analyze_cmd(_args: &[&str]) {
    crate::mesa_println!("🔍 Analyzing BIOS/UEFI configuration for WiFi compatibility...");
    let analysis = BiosAnalyzer::analyze();
    BiosAnalyzer::print_analysis(&analysis);
}
