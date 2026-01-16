/**
 * Report Parser
 *
 * Parses Discord messages to extract bug report information.
 * Supports multiple formats:
 * 1. Structured format with headers (Title:, Description:, Priority:, Component:)
 * 2. Template format with sections
 * 3. Free-form text with keyword detection
 */

export class ReportParser {
    constructor() {
        // Component keywords for auto-detection
        this.componentKeywords = {
            plugin: ['plugin', 'studio', 'roblox studio', 'luau'],
            server: ['server', 'http', 'sync', 'api', 'backend'],
            vscode: ['vscode', 'vs code', 'visual studio', 'extension'],
            core: ['core', 'serialization', 'dom', 'parsing'],
            cli: ['cli', 'command line', 'terminal'],
            mcp: ['mcp', 'ai', 'claude', 'agent'],
        };

        // Priority keywords
        this.priorityKeywords = {
            urgent: ['urgent', 'critical', 'crash', 'data loss', 'breaking'],
            high: ['high', 'important', 'blocker', 'blocking'],
            medium: ['medium', 'normal'],
            low: ['low', 'minor', 'cosmetic', 'nice to have'],
        };
    }

    /**
     * Parse a Discord message into a structured report
     * @param {Message} message - Discord.js Message object
     * @returns {Object|null} - Parsed report or null if not a valid report
     */
    parse(message) {
        const content = message.content;

        // Skip very short messages
        if (content.length < 20) {
            return null;
        }

        // Try structured format first
        const structured = this.parseStructured(content);
        if (structured) {
            return this.enrichReport(structured, message);
        }

        // Try free-form parsing
        const freeForm = this.parseFreeForm(content);
        if (freeForm) {
            return this.enrichReport(freeForm, message);
        }

        return null;
    }

    /**
     * Parse structured format with headers
     * @param {string} content - Message content
     * @returns {Object|null}
     */
    parseStructured(content) {
        const lines = content.split('\n');
        const report = {};

        let currentField = null;
        let currentValue = [];

        for (const line of lines) {
            // Check for field headers
            const headerMatch = line.match(/^(title|description|priority|component|steps|expected|actual):\s*(.*)$/i);

            if (headerMatch) {
                // Save previous field
                if (currentField) {
                    report[currentField] = currentValue.join('\n').trim();
                }

                currentField = headerMatch[1].toLowerCase();
                currentValue = headerMatch[2] ? [headerMatch[2]] : [];
            } else if (currentField) {
                // Continue current field
                currentValue.push(line);
            }
        }

        // Save last field
        if (currentField) {
            report[currentField] = currentValue.join('\n').trim();
        }

        // Must have at least a title or description
        if (!report.title && !report.description) {
            return null;
        }

        return report;
    }

    /**
     * Parse free-form text using keyword detection
     * @param {string} content - Message content
     * @returns {Object|null}
     */
    parseFreeForm(content) {
        const lowerContent = content.toLowerCase();

        // Check for bug-related keywords
        const bugKeywords = ['bug', 'issue', 'problem', 'error', 'crash', 'broken', 'not working', "doesn't work", 'failed'];
        const hasBugKeyword = bugKeywords.some(kw => lowerContent.includes(kw));

        if (!hasBugKeyword) {
            return null;
        }

        // Extract first line as title, rest as description
        const lines = content.split('\n').filter(l => l.trim());
        const title = lines[0]?.substring(0, 100) || 'Untitled Bug Report';
        const description = lines.length > 1 ? lines.slice(1).join('\n') : content;

        // Auto-detect component
        const component = this.detectComponent(lowerContent);

        // Auto-detect priority
        const priority = this.detectPriority(lowerContent);

        return {
            title,
            description,
            component,
            priority,
        };
    }

    /**
     * Detect component from content
     * @param {string} content - Lowercase content
     * @returns {string|null}
     */
    detectComponent(content) {
        for (const [component, keywords] of Object.entries(this.componentKeywords)) {
            if (keywords.some(kw => content.includes(kw))) {
                return component;
            }
        }
        return null;
    }

    /**
     * Detect priority from content
     * @param {string} content - Lowercase content
     * @returns {string|null}
     */
    detectPriority(content) {
        for (const [priority, keywords] of Object.entries(this.priorityKeywords)) {
            if (keywords.some(kw => content.includes(kw))) {
                return priority;
            }
        }
        return 'medium';
    }

    /**
     * Enrich report with message metadata
     * @param {Object} report - Parsed report
     * @param {Message} message - Discord message
     * @returns {Object}
     */
    enrichReport(report, message) {
        return {
            ...report,
            reporter: message.author.tag,
            reporterId: message.author.id,
            timestamp: message.createdAt.toISOString(),
            source: 'message',
            messageId: message.id,
            channelId: message.channel.id,
            guildId: message.guild?.id,
            messageUrl: message.url,
        };
    }
}
