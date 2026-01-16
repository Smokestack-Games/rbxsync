/**
 * Slash Command Registration
 *
 * Run this script once to register slash commands with Discord.
 * Usage: node src/commands.js
 */

import { REST, Routes, SlashCommandBuilder } from 'discord.js';
import { config } from './config.js';

const commands = [
    new SlashCommandBuilder()
        .setName('bugreport')
        .setDescription('Submit a bug report for RbxSync')
        .addStringOption(option =>
            option
                .setName('title')
                .setDescription('Brief title for the bug')
                .setRequired(true)
                .setMaxLength(100)
        )
        .addStringOption(option =>
            option
                .setName('description')
                .setDescription('Detailed description of the issue')
                .setRequired(true)
                .setMaxLength(2000)
        )
        .addStringOption(option =>
            option
                .setName('priority')
                .setDescription('How urgent is this bug?')
                .setRequired(false)
                .addChoices(
                    { name: 'Urgent - Crash/Data Loss', value: 'urgent' },
                    { name: 'High - Major Feature Broken', value: 'high' },
                    { name: 'Medium - Inconvenient', value: 'medium' },
                    { name: 'Low - Minor Issue', value: 'low' }
                )
        )
        .addStringOption(option =>
            option
                .setName('component')
                .setDescription('Which part of RbxSync is affected?')
                .setRequired(false)
                .addChoices(
                    { name: 'Plugin (Roblox Studio)', value: 'plugin' },
                    { name: 'VS Code Extension', value: 'vscode' },
                    { name: 'Server/Sync', value: 'server' },
                    { name: 'CLI', value: 'cli' },
                    { name: 'Core/Serialization', value: 'core' },
                    { name: 'MCP/AI', value: 'mcp' },
                    { name: 'Unknown', value: 'unknown' }
                )
        ),

    new SlashCommandBuilder()
        .setName('listreports')
        .setDescription('List recent bug reports (mod only)'),
];

async function registerCommands() {
    if (!config.token) {
        console.error('DISCORD_TOKEN is required');
        process.exit(1);
    }

    const rest = new REST({ version: '10' }).setToken(config.token);

    try {
        console.log('Registering slash commands...');

        // Get bot's application ID
        const me = await rest.get(Routes.oauth2CurrentApplication());

        // Register commands globally
        await rest.put(
            Routes.applicationCommands(me.id),
            { body: commands.map(c => c.toJSON()) }
        );

        console.log('Successfully registered commands!');
        console.log('Commands may take up to an hour to appear globally.');
    } catch (error) {
        console.error('Error registering commands:', error);
        process.exit(1);
    }
}

// Run if called directly
if (import.meta.url === `file://${process.argv[1]}`) {
    registerCommands();
}

export { commands, registerCommands };
