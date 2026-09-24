import { Command } from 'commander';
import { HardwareWalletDiagnostics } from '../diagnostics/hardwareWalletDiagnostics';
import chalk from 'chalk'; // Assumes styling framework is present in StarForge core

export const registerDiagnosticsCommand = (program: Command) => {
  program
    .command('diagnostics')
    .description('Run standard integration suite tests for attached Ledger/Trezor devices')
    .option('-w, --wallet <type>', 'Specify isolated hardware target assessment ("ledger" or "trezor")')
    .action(async (options) => {
      console.log(chalk.blue('🚀 Initiating StarForge Hardware Wallet Connectivity Diagnostics...\n'));

      const results = await HardwareWalletDiagnostics.runLiveDiagnostics();
      const filteredResults = options.wallet 
        ? results.filter(r => r.walletType.toLowerCase() === options.wallet.toLowerCase())
        : results;

      if (filteredResults.length === 0) {
        console.log(chalk.yellow('ℹ No diagnostics profiles filtered matches your query criteria.'));
        return;
      }

      filteredResults.forEach((report) => {
        const header = `[${report.walletType.toUpperCase()}] Diagnostic Probe`;
        if (report.status === 'SUCCESS') {
          console.log(chalk.green(`✔ ${header}: SUCCESS`));
          console.log(`  Message: ${report.message}\n`);
        } else if (report.status === 'WARNING') {
          console.log(chalk.yellow(`⚠ ${header}: WARNING`));
          console.log(`  Message: ${report.message}`);
          console.log(`  Remediation: ${report.remediation}\n`);
        } else {
          console.log(chalk.red(`✘ ${header}: FAILURE (${report.issueType})`));
          console.log(`  Error Details: ${report.message}`);
          console.log(chalk.cyan(`  Fix Action Required: ${report.remediation}\n`));
        }
      });
    });
};