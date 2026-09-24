import { HardwareWalletDiagnostics } from './hardwareWalletDiagnostics';
import { Command } from 'commander';
import chalk from 'chalk';

const program = new Command();

program
  .option('-w, --wallet <type>', 'Specify isolated hardware target assessment ("ledger" or "trezor")')
  .action(async (options) => {
    const results = await HardwareWalletDiagnostics.runLiveDiagnostics();
    const filteredResults = options.wallet 
      ? results.filter(r => r.walletType.toLowerCase() === options.wallet.toLowerCase())
      : results;

    if (filteredResults.length === 0) {
      console.log(chalk.yellow('ℹ No diagnostics profiles matches your query criteria.'));
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

program.parse(process.argv);