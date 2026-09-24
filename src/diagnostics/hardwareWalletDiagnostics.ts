import { TransportError } from '@ledgerhq/errors'; // Common Ledger Transport package

export interface DiagnosticReport {
  status: 'SUCCESS' | 'FAILURE' | 'WARNING';
  walletType: 'Ledger' | 'Trezor' | 'Unknown';
  issueType?: 'DRIVER_MISSING' | 'USB_PERMISSIONS' | 'UNSUPPORTED_HD_PATH' | 'FIRMWARE_INCOMPATIBLE' | 'UNKNOWN';
  message: string;
  remediation: string;
}

export class HardwareWalletDiagnostics {
  
  /**
   * Analyzes raw connection/operation errors thrown by hardware wallets and translates
   * them into human-readable diagnostics and actionable remediation steps.
   */
  public static analyzeError(error: any, walletHint: 'Ledger' | 'Trezor' | 'Unknown' = 'Unknown'): DiagnosticReport {
    const errorStr = String(error.message || error).toLowerCase();
    const errorCode = error.statusCode || error.code || '';

    // --- 1. DETECT LEDGER SPECIFIC ERRORS ---
    if (walletHint === 'Ledger' || errorStr.includes('ledger') || errorStr.includes('hid')) {
      
      // Missing device drivers / Device not found
      if (errorStr.includes('device not found') || errorStr.includes('cannot open device') || errorStr.includes('unable to open device')) {
        return {
          status: 'FAILURE',
          walletType: 'Ledger',
          issueType: 'DRIVER_MISSING',
          message: 'Failed to locate or open the Ledger device.',
          remediation: 'Ensure your Ledger is connected via USB, unlocked with your PIN, and that the appropriate App (e.g., Stellar) is open. On Windows, verify that the device drivers are up-to-date in Device Manager.'
        };
      }

      // Incorrect USB Permissions (Common on Linux/Ubuntu)
      if (errorStr.includes('access denied') || errorStr.includes('insufficient permissions') || errorStr.includes('udev') || errorCode === 0x6801) {
        return {
          status: 'FAILURE',
          walletType: 'Ledger',
          issueType: 'USB_PERMISSIONS',
          message: 'Insufficient USB access permissions to interface with the Ledger device.',
          remediation: 'Linux Users: Ensure your udev rules are properly set up. Run the following command or add rules manually: \n"wget -q -O - https://raw.githubusercontent.com/LedgerHQ/udev-rules/master/add_udev_rules.sh | sudo bash"'
        };
      }

      // Unsupported HD Paths
      if (errorStr.includes('invalid path') || errorStr.includes('derivation path') || errorCode === 0x6a80 || errorStr.includes('rejected by user')) {
        return {
          status: 'FAILURE',
          walletType: 'Ledger',
          issueType: 'UNSUPPORTED_HD_PATH',
          message: 'The requested Hierarchical Deterministic (HD) derivation path is unsupported or was explicitly rejected.',
          remediation: 'Verify that your configuration specifies a compliant BIP44 path (e.g., 44\'/148\'/0\' for Stellar). If using custom paths, verify that "Expert Mode" or "Custom Paths" is enabled inside your Ledger app settings.'
        };
      }

      // Incompatible Firmware
      if (errorStr.includes('outdated') || errorStr.includes('not supported') || errorStr.includes('version') || errorCode === 0x5515) {
        return {
          status: 'FAILURE',
          walletType: 'Ledger',
          issueType: 'FIRMWARE_INCOMPATIBLE',
          message: 'Incompatible or outdated Ledger firmware / App version detected.',
          remediation: 'Open Ledger Live, navigate to "Manager", update your Ledger device firmware to the latest version, and ensure your on-device Wallet Application is fully updated.'
        };
      }
    }

    // --- 2. DETECT TREZOR SPECIFIC ERRORS ---
    if (walletHint === 'Trezor' || errorStr.includes('trezor') || errorStr.includes('bridge')) {
      
      // Missing Trezor Bridge / Drivers
      if (errorStr.includes('bridge not found') || errorStr.includes('connect_error') || errorStr.includes('failed to fetch')) {
        return {
          status: 'FAILURE',
          walletType: 'Trezor',
          issueType: 'DRIVER_MISSING',
          message: 'Could not establish connection with Trezor Bridge.',
          remediation: 'Ensure that Trezor Bridge is installed and running on your system (check http://localhost:21325/). Alternatively, switch to a WebUSB-compatible browser environment if using a web interface.'
        };
      }

      // USB Permissions
      if (errorStr.includes('device used by another application') || errorStr.includes('permission denied') || errorStr.includes('usb claim')) {
        return {
          status: 'FAILURE',
          walletType: 'Trezor',
          issueType: 'USB_PERMISSIONS',
          message: 'Trezor USB channel is blocked or lacks appropriate operational permissions.',
          remediation: 'Close other applications interfacing with your Trezor (such as Trezor Suite or another browser tab). Linux users should install Trezor udev rules.'
        };
      }

      // Unsupported HD Paths
      if (errorStr.includes('invalid path') || errorStr.includes('forbidden path') || errorStr.includes('failure_invalid_bip32_path')) {
        return {
          status: 'FAILURE',
          walletType: 'Trezor',
          issueType: 'UNSUPPORTED_HD_PATH',
          message: 'Trezor rejected the requested HD path.',
          remediation: 'Ensure the derivation path meets standard structures matching Trezor guidelines for the designated network token standard.'
        };
      }
    }

    // Generic fallback mapping
    return {
      status: 'FAILURE',
      walletType: walletHint,
      issueType: 'UNKNOWN',
      message: error.message || String(error),
      remediation: 'Verify physical connectivity, unlock the hardware wallet screen, ensure matching app protocols are open, and retry.'
    };
  }

  /**
   * Proactively triggers a live diagnostic sweep to probe for attached USB hardware wallets.
   */
  public static async runLiveDiagnostics(): Promise<DiagnosticReport[]> {
    const reports: DiagnosticReport[] = [];

    // Probe Ledger
    try {
      // Lazy-load to avoid breaking non-node environments
      const TransportNodeHid = require('@ledgerhq/transport-node-hid').default;
      const devices = await TransportNodeHid.list();
      if (devices.length === 0) {
        reports.push({
          status: 'WARNING',
          walletType: 'Ledger',
          message: 'No physical Ledger USB devices found during automated scan.',
          remediation: 'Plug your Ledger device via a stable USB connection directly to your PC and enter your PIN code.'
        });
      } else {
        reports.push({
          status: 'SUCCESS',
          walletType: 'Ledger',
          message: `Detected ${devices.length} Ledger USB interface device(s) successfully.`,
          remediation: 'No action needed. Ready for connection pairing.'
        });
      }
    } catch (err) {
      reports.push(this.analyzeError(err, 'Ledger'));
    }

    // Probe Trezor
    try {
      const TrezorConnect = require('@trezor/connect').default;
      // Initialize configuration parameters for diagnostic probes safely
      await TrezorConnect.init({
        lazyLoad: true,
        manifest: {
          email: 'developer@starforge.com',
          appUrl: 'https://github.com/Nanle-code/StarForge'
        }
      });
      
      const features = await TrezorConnect.getFeatures();
      if (features.success) {
        reports.push({
          status: 'SUCCESS',
          walletType: 'Trezor',
          message: `Connected successfully to Trezor ${features.payload.model} (Firmware v${features.payload.major_version}.${features.payload.minor_version}.${features.payload.patch_version})`,
          remediation: 'No action needed.'
        });
      } else {
        reports.push(this.analyzeError(features.payload.error, 'Trezor'));
      }
    } catch (err) {
      reports.push(this.analyzeError(err, 'Trezor'));
    }

    return reports;
  }
}