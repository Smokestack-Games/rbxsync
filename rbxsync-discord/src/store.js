/**
 * Report Store
 *
 * Persists bug reports to the filesystem as JSON files.
 * Each report is saved as a separate file for easy consumption.
 */

import { existsSync, mkdirSync, writeFileSync, readFileSync, readdirSync } from 'fs';
import { join } from 'path';

export class ReportStore {
    constructor(reportsDir) {
        this.reportsDir = reportsDir;

        // Ensure reports directory exists
        if (!existsSync(this.reportsDir)) {
            mkdirSync(this.reportsDir, { recursive: true });
        }
    }

    /**
     * Generate a unique report ID
     * Format: RBXFB-YYYYMMDD-XXXX (e.g., RBXFB-20260116-0001)
     * @returns {string}
     */
    generateId() {
        const date = new Date();
        const dateStr = date.toISOString().slice(0, 10).replace(/-/g, '');

        // Get existing reports for today to determine sequence
        const todayPattern = `RBXFB-${dateStr}`;
        const existing = readdirSync(this.reportsDir)
            .filter(f => f.startsWith(todayPattern))
            .length;

        const sequence = String(existing + 1).padStart(4, '0');
        return `${todayPattern}-${sequence}`;
    }

    /**
     * Save a report to the store
     * @param {Object} report - Report data
     * @returns {Object} - Saved report with ID
     */
    async save(report) {
        const id = this.generateId();
        const filename = `${id}.json`;
        const filepath = join(this.reportsDir, filename);

        const savedReport = {
            id,
            ...report,
            savedAt: new Date().toISOString(),
        };

        writeFileSync(filepath, JSON.stringify(savedReport, null, 2));

        // Also update the index file for quick lookups
        await this.updateIndex(savedReport);

        return savedReport;
    }

    /**
     * Update the index file with a new report
     * @param {Object} report - Report to add to index
     */
    async updateIndex(report) {
        const indexPath = join(this.reportsDir, 'index.json');
        let index = { reports: [], lastUpdated: null };

        if (existsSync(indexPath)) {
            try {
                index = JSON.parse(readFileSync(indexPath, 'utf-8'));
            } catch (e) {
                // Reset index if corrupted
                index = { reports: [], lastUpdated: null };
            }
        }

        // Add summary to index
        index.reports.unshift({
            id: report.id,
            title: report.title,
            priority: report.priority,
            component: report.component,
            reporter: report.reporter,
            timestamp: report.timestamp,
            status: 'new',
        });

        // Keep only last 1000 reports in index
        if (index.reports.length > 1000) {
            index.reports = index.reports.slice(0, 1000);
        }

        index.lastUpdated = new Date().toISOString();

        writeFileSync(indexPath, JSON.stringify(index, null, 2));
    }

    /**
     * Get a report by ID
     * @param {string} id - Report ID
     * @returns {Object|null}
     */
    get(id) {
        const filepath = join(this.reportsDir, `${id}.json`);

        if (!existsSync(filepath)) {
            return null;
        }

        return JSON.parse(readFileSync(filepath, 'utf-8'));
    }

    /**
     * List recent reports
     * @param {number} limit - Maximum number of reports to return
     * @returns {Array}
     */
    async list(limit = 10) {
        const indexPath = join(this.reportsDir, 'index.json');

        if (!existsSync(indexPath)) {
            return [];
        }

        try {
            const index = JSON.parse(readFileSync(indexPath, 'utf-8'));
            return index.reports.slice(0, limit);
        } catch (e) {
            return [];
        }
    }

    /**
     * Export all reports in Linear-compatible format
     * @returns {Array}
     */
    exportForLinear() {
        const indexPath = join(this.reportsDir, 'index.json');

        if (!existsSync(indexPath)) {
            return [];
        }

        try {
            const index = JSON.parse(readFileSync(indexPath, 'utf-8'));

            return index.reports
                .filter(r => r.status === 'new')
                .map(summary => {
                    const full = this.get(summary.id);
                    if (!full) return null;

                    return {
                        title: full.title || 'Discord Bug Report',
                        description: this.formatLinearDescription(full),
                        priority: this.mapPriorityToLinear(full.priority),
                        labels: full.component ? [full.component] : [],
                    };
                })
                .filter(Boolean);
        } catch (e) {
            return [];
        }
    }

    /**
     * Format report description for Linear
     * @param {Object} report - Full report
     * @returns {string}
     */
    formatLinearDescription(report) {
        const parts = [
            report.description,
            '',
            '---',
            `**Reporter:** ${report.reporter}`,
            `**Source:** Discord (${report.source})`,
            `**Report ID:** ${report.id}`,
        ];

        if (report.messageUrl) {
            parts.push(`**Message:** ${report.messageUrl}`);
        }

        if (report.steps) {
            parts.unshift('', '**Steps to Reproduce:**', report.steps);
        }

        if (report.expected) {
            parts.unshift('', '**Expected:**', report.expected);
        }

        if (report.actual) {
            parts.unshift('', '**Actual:**', report.actual);
        }

        return parts.join('\n');
    }

    /**
     * Map priority string to Linear priority number
     * @param {string} priority
     * @returns {number}
     */
    mapPriorityToLinear(priority) {
        const map = {
            urgent: 1,
            high: 2,
            medium: 3,
            low: 4,
        };
        return map[priority] || 3;
    }
}
