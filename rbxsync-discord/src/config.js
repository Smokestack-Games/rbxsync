/**
 * Configuration loader for the Discord bot
 */

import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { existsSync, readFileSync } from 'fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const projectRoot = join(__dirname, '..');

// Load .env file if it exists
const envPath = join(projectRoot, '.env');
if (existsSync(envPath)) {
    const envContent = readFileSync(envPath, 'utf-8');
    for (const line of envContent.split('\n')) {
        const trimmed = line.trim();
        if (trimmed && !trimmed.startsWith('#')) {
            const [key, ...valueParts] = trimmed.split('=');
            const value = valueParts.join('=');
            if (key && value) {
                process.env[key.trim()] = value.trim();
            }
        }
    }
}

export const config = {
    // Discord bot token
    token: process.env.DISCORD_TOKEN,

    // Channel ID to monitor for bug reports
    bugReportChannelId: process.env.BUG_REPORT_CHANNEL_ID,

    // Optional webhook URL to forward reports
    webhookUrl: process.env.WEBHOOK_URL,

    // Directory to save reports
    reportsDir: process.env.REPORTS_DIR || join(projectRoot, 'reports'),
};

// Validate required configuration
if (!config.token) {
    console.error('[Config] DISCORD_TOKEN is required. Set it in your .env file.');
    process.exit(1);
}
