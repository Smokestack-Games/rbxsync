require('dotenv').config();
const { Client, GatewayIntentBits, EmbedBuilder, SlashCommandBuilder, REST, Routes, ModalBuilder, TextInputBuilder, TextInputStyle, ActionRowBuilder, ButtonBuilder, ButtonStyle } = require('discord.js');
const fs = require('fs');
const path = require('path');

const client = new Client({
    intents: [
        GatewayIntentBits.Guilds,
        GatewayIntentBits.GuildMessages,
        GatewayIntentBits.MessageContent,
    ],
});

const REPORTS_DIR = path.join(__dirname, '../reports');

// Ensure reports directory exists
if (!fs.existsSync(REPORTS_DIR)) {
    fs.mkdirSync(REPORTS_DIR, { recursive: true });
}

// Slash commands definition
const commands = [
    new SlashCommandBuilder()
        .setName('bug')
        .setDescription('Report a bug in RbxSync'),
    new SlashCommandBuilder()
        .setName('feedback')
        .setDescription('Submit feedback about RbxSync'),
    new SlashCommandBuilder()
        .setName('feature')
        .setDescription('Request a new feature for RbxSync'),
].map(command => command.toJSON());

// Register slash commands when bot is ready
client.once('ready', async () => {
    console.log(`Logged in as ${client.user.tag}`);

    const rest = new REST({ version: '10' }).setToken(process.env.DISCORD_TOKEN);

    try {
        console.log('Registering slash commands...');
        await rest.put(
            Routes.applicationCommands(client.user.id),
            { body: commands },
        );
        console.log('Slash commands registered successfully');
    } catch (error) {
        console.error('Error registering commands:', error);
    }
});

// Handle slash commands
client.on('interactionCreate', async (interaction) => {
    if (interaction.isChatInputCommand()) {
        await handleSlashCommand(interaction);
    } else if (interaction.isModalSubmit()) {
        await handleModalSubmit(interaction);
    } else if (interaction.isButton()) {
        await handleButton(interaction);
    }
});

async function handleSlashCommand(interaction) {
    const { commandName } = interaction;

    if (commandName === 'bug') {
        const modal = new ModalBuilder()
            .setCustomId('bug_report_modal')
            .setTitle('Bug Report');

        const titleInput = new TextInputBuilder()
            .setCustomId('bug_title')
            .setLabel('Bug Title')
            .setPlaceholder('Brief description of the bug')
            .setStyle(TextInputStyle.Short)
            .setRequired(true)
            .setMaxLength(100);

        const descriptionInput = new TextInputBuilder()
            .setCustomId('bug_description')
            .setLabel('Description')
            .setPlaceholder('What happened? What did you expect to happen?')
            .setStyle(TextInputStyle.Paragraph)
            .setRequired(true);

        const stepsInput = new TextInputBuilder()
            .setCustomId('bug_steps')
            .setLabel('Steps to Reproduce')
            .setPlaceholder('1. Do this\n2. Then this\n3. Bug occurs')
            .setStyle(TextInputStyle.Paragraph)
            .setRequired(false);

        const versionInput = new TextInputBuilder()
            .setCustomId('bug_version')
            .setLabel('RbxSync Version')
            .setPlaceholder('e.g., v1.1.2')
            .setStyle(TextInputStyle.Short)
            .setRequired(false);

        modal.addComponents(
            new ActionRowBuilder().addComponents(titleInput),
            new ActionRowBuilder().addComponents(descriptionInput),
            new ActionRowBuilder().addComponents(stepsInput),
            new ActionRowBuilder().addComponents(versionInput),
        );

        await interaction.showModal(modal);
    } else if (commandName === 'feedback') {
        const modal = new ModalBuilder()
            .setCustomId('feedback_modal')
            .setTitle('Submit Feedback');

        const typeInput = new TextInputBuilder()
            .setCustomId('feedback_type')
            .setLabel('Feedback Type')
            .setPlaceholder('General / UX / Performance / Documentation')
            .setStyle(TextInputStyle.Short)
            .setRequired(true);

        const contentInput = new TextInputBuilder()
            .setCustomId('feedback_content')
            .setLabel('Your Feedback')
            .setPlaceholder('Share your thoughts about RbxSync...')
            .setStyle(TextInputStyle.Paragraph)
            .setRequired(true);

        modal.addComponents(
            new ActionRowBuilder().addComponents(typeInput),
            new ActionRowBuilder().addComponents(contentInput),
        );

        await interaction.showModal(modal);
    } else if (commandName === 'feature') {
        const modal = new ModalBuilder()
            .setCustomId('feature_modal')
            .setTitle('Feature Request');

        const titleInput = new TextInputBuilder()
            .setCustomId('feature_title')
            .setLabel('Feature Title')
            .setPlaceholder('Brief name for the feature')
            .setStyle(TextInputStyle.Short)
            .setRequired(true)
            .setMaxLength(100);

        const descriptionInput = new TextInputBuilder()
            .setCustomId('feature_description')
            .setLabel('Description')
            .setPlaceholder('Describe the feature you want...')
            .setStyle(TextInputStyle.Paragraph)
            .setRequired(true);

        const useCaseInput = new TextInputBuilder()
            .setCustomId('feature_usecase')
            .setLabel('Use Case')
            .setPlaceholder('How would this feature help you?')
            .setStyle(TextInputStyle.Paragraph)
            .setRequired(false);

        modal.addComponents(
            new ActionRowBuilder().addComponents(titleInput),
            new ActionRowBuilder().addComponents(descriptionInput),
            new ActionRowBuilder().addComponents(useCaseInput),
        );

        await interaction.showModal(modal);
    }
}

async function handleModalSubmit(interaction) {
    const { customId } = interaction;

    if (customId === 'bug_report_modal') {
        const title = interaction.fields.getTextInputValue('bug_title');
        const description = interaction.fields.getTextInputValue('bug_description');
        const steps = interaction.fields.getTextInputValue('bug_steps') || 'Not provided';
        const version = interaction.fields.getTextInputValue('bug_version') || 'Unknown';

        const report = createBugReport(interaction.user, title, description, steps, version);
        await saveReport('bug', report);

        const embed = new EmbedBuilder()
            .setColor(0xFF0000)
            .setTitle('Bug Report Submitted')
            .setDescription(`**${title}**`)
            .addFields(
                { name: 'Description', value: truncate(description, 1024) },
                { name: 'Steps to Reproduce', value: truncate(steps, 1024) },
                { name: 'Version', value: version, inline: true },
                { name: 'Submitted by', value: interaction.user.tag, inline: true },
                { name: 'Report ID', value: report.id, inline: true },
            )
            .setTimestamp();

        await interaction.reply({ embeds: [embed] });

        // Send to reports channel if configured
        await sendToReportsChannel(interaction.guild, embed);
    } else if (customId === 'feedback_modal') {
        const type = interaction.fields.getTextInputValue('feedback_type');
        const content = interaction.fields.getTextInputValue('feedback_content');

        const report = createFeedbackReport(interaction.user, type, content);
        await saveReport('feedback', report);

        const embed = new EmbedBuilder()
            .setColor(0x00FF00)
            .setTitle('Feedback Submitted')
            .addFields(
                { name: 'Type', value: type, inline: true },
                { name: 'Submitted by', value: interaction.user.tag, inline: true },
                { name: 'Report ID', value: report.id, inline: true },
                { name: 'Feedback', value: truncate(content, 1024) },
            )
            .setTimestamp();

        await interaction.reply({ embeds: [embed] });
        await sendToReportsChannel(interaction.guild, embed);
    } else if (customId === 'feature_modal') {
        const title = interaction.fields.getTextInputValue('feature_title');
        const description = interaction.fields.getTextInputValue('feature_description');
        const useCase = interaction.fields.getTextInputValue('feature_usecase') || 'Not provided';

        const report = createFeatureRequest(interaction.user, title, description, useCase);
        await saveReport('feature', report);

        const embed = new EmbedBuilder()
            .setColor(0x0099FF)
            .setTitle('Feature Request Submitted')
            .setDescription(`**${title}**`)
            .addFields(
                { name: 'Description', value: truncate(description, 1024) },
                { name: 'Use Case', value: truncate(useCase, 1024) },
                { name: 'Submitted by', value: interaction.user.tag, inline: true },
                { name: 'Report ID', value: report.id, inline: true },
            )
            .setTimestamp();

        await interaction.reply({ embeds: [embed] });
        await sendToReportsChannel(interaction.guild, embed);
    }
}

async function handleButton(interaction) {
    // Reserved for future button interactions (e.g., marking reports as resolved)
    await interaction.reply({ content: 'Button interaction received', ephemeral: true });
}

// Listen for messages in bug report channels
client.on('messageCreate', async (message) => {
    if (message.author.bot) return;

    // Check if message is in a designated bug report channel
    const bugChannelNames = ['bug-reports', 'bugs', 'rbxsync-bugs'];
    const feedbackChannelNames = ['feedback', 'suggestions', 'rbxsync-feedback'];

    const channelName = message.channel.name?.toLowerCase();

    if (bugChannelNames.includes(channelName)) {
        // Auto-format bug report from plain text
        const report = createBugReportFromMessage(message);
        await saveReport('bug', report);

        const embed = new EmbedBuilder()
            .setColor(0xFF0000)
            .setTitle('Bug Report Logged')
            .setDescription(truncate(message.content, 2048))
            .addFields(
                { name: 'Submitted by', value: message.author.tag, inline: true },
                { name: 'Report ID', value: report.id, inline: true },
            )
            .setTimestamp();

        await message.reply({ embeds: [embed] });
    } else if (feedbackChannelNames.includes(channelName)) {
        // Auto-format feedback from plain text
        const report = createFeedbackFromMessage(message);
        await saveReport('feedback', report);

        const embed = new EmbedBuilder()
            .setColor(0x00FF00)
            .setTitle('Feedback Logged')
            .setDescription(truncate(message.content, 2048))
            .addFields(
                { name: 'Submitted by', value: message.author.tag, inline: true },
                { name: 'Report ID', value: report.id, inline: true },
            )
            .setTimestamp();

        await message.reply({ embeds: [embed] });
    }
});

// Helper functions
function generateReportId() {
    const timestamp = Date.now().toString(36);
    const random = Math.random().toString(36).substring(2, 6);
    return `RPT-${timestamp}-${random}`.toUpperCase();
}

function createBugReport(user, title, description, steps, version) {
    return {
        id: generateReportId(),
        type: 'bug',
        title,
        description,
        stepsToReproduce: steps,
        version,
        reporter: {
            id: user.id,
            tag: user.tag,
        },
        createdAt: new Date().toISOString(),
        status: 'open',
    };
}

function createBugReportFromMessage(message) {
    const lines = message.content.split('\n');
    const title = lines[0].substring(0, 100);
    const description = lines.slice(1).join('\n') || lines[0];

    return {
        id: generateReportId(),
        type: 'bug',
        title,
        description,
        stepsToReproduce: 'Parsed from message',
        version: 'Unknown',
        reporter: {
            id: message.author.id,
            tag: message.author.tag,
        },
        messageId: message.id,
        channelId: message.channel.id,
        createdAt: new Date().toISOString(),
        status: 'open',
    };
}

function createFeedbackReport(user, type, content) {
    return {
        id: generateReportId(),
        type: 'feedback',
        feedbackType: type,
        content,
        reporter: {
            id: user.id,
            tag: user.tag,
        },
        createdAt: new Date().toISOString(),
        status: 'open',
    };
}

function createFeedbackFromMessage(message) {
    return {
        id: generateReportId(),
        type: 'feedback',
        feedbackType: 'General',
        content: message.content,
        reporter: {
            id: message.author.id,
            tag: message.author.tag,
        },
        messageId: message.id,
        channelId: message.channel.id,
        createdAt: new Date().toISOString(),
        status: 'open',
    };
}

function createFeatureRequest(user, title, description, useCase) {
    return {
        id: generateReportId(),
        type: 'feature',
        title,
        description,
        useCase,
        reporter: {
            id: user.id,
            tag: user.tag,
        },
        createdAt: new Date().toISOString(),
        status: 'open',
    };
}

async function saveReport(type, report) {
    const filename = `${type}-${report.id}.json`;
    const filepath = path.join(REPORTS_DIR, filename);

    try {
        fs.writeFileSync(filepath, JSON.stringify(report, null, 2));
        console.log(`Report saved: ${filename}`);
    } catch (error) {
        console.error('Error saving report:', error);
    }
}

async function sendToReportsChannel(guild, embed) {
    if (!guild || !process.env.REPORTS_CHANNEL_ID) return;

    try {
        const channel = await guild.channels.fetch(process.env.REPORTS_CHANNEL_ID);
        if (channel) {
            await channel.send({ embeds: [embed] });
        }
    } catch (error) {
        console.error('Error sending to reports channel:', error);
    }
}

function truncate(str, maxLength) {
    if (!str) return 'N/A';
    if (str.length <= maxLength) return str;
    return str.substring(0, maxLength - 3) + '...';
}

// Login
const token = process.env.DISCORD_TOKEN;
if (!token) {
    console.error('Error: DISCORD_TOKEN environment variable is not set');
    console.error('Create a .env file with DISCORD_TOKEN=your_bot_token');
    process.exit(1);
}

client.login(token);
