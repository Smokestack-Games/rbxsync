# SEO Strategy for rbxsync.dev

**Date:** 2026-01-16
**Issue:** RBXSYNC-8
**Purpose:** Improve organic search visibility for rbxsync.dev documentation

---

## Executive Summary

RbxSync has solid documentation content but lacks SEO optimization. The current site has generic page titles, no per-page meta descriptions, and misses key search terms Roblox developers use. With targeted improvements, RbxSync can capture traffic from developers searching for Rojo alternatives, AI-powered Roblox development, and two-way sync tools.

---

## 1. Current SEO Gaps

### 1.1 Page Titles

**Issue:** All pages use the generic pattern `{PageTitle} | RbxSync Docs`

**Current Examples:**
- "Introduction | RbxSync Docs"
- "CLI Overview | RbxSync Docs"
- "FAQ | RbxSync Docs"

**Problems:**
- Missing high-value keywords like "Roblox", "VS Code", "sync"
- Generic titles don't differentiate from competitors
- No action-oriented or benefit-driven language

### 1.2 Meta Descriptions

**Issue:** Single global description used site-wide

**Current:**
```
Documentation for RbxSync – two-way sync between Roblox Studio and VS Code.
```

**Problems:**
- All pages share the same meta description
- Doesn't mention key features (AI, MCP, git, zero-config)
- No page-specific descriptions for FAQ, installation, etc.

### 1.3 H1/H2 Headings

**Issue:** Headings are too generic and miss keyword opportunities

**Examples of weak headings:**
- "Introduction" (should be: "Introduction to RbxSync: Two-Way Roblox Studio Sync")
- "Features" (should be: "RbxSync Features: AI Integration, Full Property Sync, Git Support")
- "Quick Start" (should be: "Quick Start: Sync Roblox Studio with VS Code in 5 Minutes")

### 1.4 Missing Content

**Critical gaps:**
- No comparison pages (vs Rojo, vs Argon)
- No "Rojo alternative" or "Rojo migration" content
- No tutorial/guide content for common workflows
- No blog or changelog that could rank for long-tail queries

### 1.5 Technical SEO

**Issues identified:**
- Homepage is a redirect (loses SEO juice)
- No sitemap.xml found
- No robots.txt found
- No structured data/JSON-LD
- Missing og:image on most pages

---

## 2. Target Keywords

### 2.1 Primary Keywords (High Priority)

| Keyword | Monthly Search Volume (Est.) | Competition | RbxSync Relevance |
|---------|------------------------------|-------------|-------------------|
| roblox vscode sync | Medium | Medium | Core feature |
| roblox studio vs code | Medium | Medium | Core feature |
| rojo alternative | Low-Medium | Low | Key differentiator |
| roblox git version control | Low-Medium | Low | Core feature |
| two-way sync roblox | Low | Very Low | Unique feature |

### 2.2 Secondary Keywords (AI-Focused)

| Keyword | Monthly Search Volume (Est.) | Competition | RbxSync Relevance |
|---------|------------------------------|-------------|-------------------|
| roblox mcp | Growing | Very Low | Unique feature |
| roblox ai development | Growing | Low | Key differentiator |
| claude roblox | Growing | Very Low | MCP integration |
| roblox studio ai assistant | Growing | Low | MCP integration |

### 2.3 Long-Tail Keywords

| Keyword Phrase | Intent |
|----------------|--------|
| "how to use vscode with roblox" | Tutorial seekers |
| "roblox studio external editor" | Feature seekers |
| "sync roblox scripts to git" | Workflow seekers |
| "roblox luau lsp vscode" | Developer tooling |
| "roblox studio two way sync" | Feature seekers |
| "rojo vs argon" | Comparison shoppers |
| "extract roblox game to files" | Feature seekers |
| "roblox ci/cd pipeline" | Advanced users |

### 2.4 Competitor-Targeted Keywords

| Keyword | Strategy |
|---------|----------|
| "rojo sync problems" | Capture frustrated users |
| "rojo syncback not working" | Solve pain points |
| "argon vs rojo" | Comparison page opportunity |
| "better than rojo" | Direct comparison content |
| "rojo alternative 2026" | Capture seekers |

---

## 3. Recommended Page Title Changes

### VitePress Config Updates

```typescript
// docs/.vitepress/config.ts
export default defineConfig({
  title: 'RbxSync',
  titleTemplate: ':title | RbxSync - Roblox Studio Sync Tool',
  // ...
})
```

### Per-Page Title Recommendations

| Page | Current Title | Recommended Title |
|------|---------------|-------------------|
| Getting Started | Introduction | RbxSync: Two-Way Sync Between Roblox Studio and VS Code |
| Installation | Installation | Install RbxSync - Sync Roblox Studio with VS Code and Git |
| Quick Start | Quick Start | Quick Start: Sync Your Roblox Game in 5 Minutes |
| CLI Overview | CLI Overview | RbxSync CLI Reference - Commands for Roblox Sync |
| Plugin | Studio Plugin | RbxSync Studio Plugin - Two-Way Sync for Roblox |
| VS Code | VS Code Extension | RbxSync for VS Code - Roblox Studio Integration |
| MCP | MCP Integration | RbxSync MCP: AI-Powered Roblox Development with Claude |
| FAQ | FAQ | RbxSync FAQ - Common Questions About Roblox Sync |
| File Formats | File Formats | RbxSync File Formats - .luau, .rbxjson Reference |

---

## 4. Recommended Meta Description Changes

### VitePress Per-Page Frontmatter

Add frontmatter to each markdown file:

```yaml
---
title: Install RbxSync
description: Install RbxSync CLI, Studio plugin, and VS Code extension. Sync Roblox Studio with VS Code and Git in minutes. Supports macOS and Windows.
---
```

### Recommended Meta Descriptions by Page

| Page | Recommended Description (Max 160 chars) |
|------|----------------------------------------|
| Getting Started | RbxSync enables two-way sync between Roblox Studio and VS Code. Edit in Studio or VS Code - changes sync automatically. Git integration included. |
| Installation | Install RbxSync CLI, Studio plugin, and VS Code extension. Quick setup for macOS and Windows. Start syncing your Roblox game in minutes. |
| Quick Start | Create your first RbxSync project in 5 minutes. Extract your Roblox game to git-friendly files and start syncing with VS Code. |
| CLI | RbxSync CLI reference. Commands for extracting, syncing, building, and managing Roblox Studio projects from the command line. |
| Plugin | RbxSync Studio plugin enables real-time two-way sync. Extract games to files, sync changes automatically, stream console output. |
| VS Code | RbxSync VS Code extension for Roblox development. Console streaming, E2E testing, and seamless sync with Roblox Studio. |
| MCP | RbxSync MCP server enables AI-assisted Roblox development. Let Claude or Cursor write, test, and debug your game automatically. |
| FAQ | Common questions about RbxSync. Installation help, sync issues, plugin troubleshooting, and more. |
| File Formats | RbxSync file format reference. How .luau scripts and .rbxjson property files work for Roblox Studio sync. |

---

## 5. Content Opportunities

### 5.1 High-Priority Pages to Create

| Page | Target Keywords | Purpose |
|------|-----------------|---------|
| `/comparison/` | rojo alternative, rojo vs argon vs rbxsync | Capture comparison shoppers |
| `/migrate-from-rojo/` | migrate from rojo, rojo to rbxsync | Convert Rojo users |
| `/why-rbxsync/` | why rbxsync, roblox sync tool | Landing page for organic |
| `/tutorials/` index | roblox vscode tutorial | Hub for tutorial content |

### 5.2 Tutorial/Guide Content

| Topic | Target Keywords |
|-------|-----------------|
| Setting up Luau LSP with RbxSync | luau lsp vscode roblox, luau autocomplete |
| Git workflow for Roblox games | roblox git workflow, version control roblox |
| CI/CD with RbxSync and GitHub Actions | roblox ci cd, roblox github actions |
| Using Claude to build Roblox games | roblox ai development, claude roblox |
| Team collaboration with RbxSync | roblox team development, roblox collaboration |

### 5.3 Blog/Changelog

**Recommendation:** Add a `/blog/` or `/changelog/` section

**Content Ideas:**
- "RbxSync v1.2: What's New in AI Integration"
- "Why We Built RbxSync: The Problems with Rojo's Two-Way Sync"
- "How Top Studios Use External Editors for Roblox Development"
- "The Future of AI-Assisted Roblox Development"

---

## 6. Technical SEO Fixes

### 6.1 Homepage Redirect Issue

**Current:** `/` redirects to `/getting-started/`

**Problem:** Search engines may not properly index the homepage. Link equity is lost.

**Recommendation:** Create a proper landing page at `/` with:
- Clear value proposition
- Key features
- Call-to-action buttons
- Proper H1 with "RbxSync" + target keywords

### 6.2 Add sitemap.xml

VitePress should generate this automatically, but verify it exists at:
```
https://rbxsync.dev/sitemap.xml
```

Add to `config.ts`:
```typescript
sitemap: {
  hostname: 'https://rbxsync.dev'
}
```

### 6.3 Add robots.txt

Create `/docs/public/robots.txt`:
```
User-agent: *
Allow: /

Sitemap: https://rbxsync.dev/sitemap.xml
```

### 6.4 Add Structured Data

Add JSON-LD for:
- **Organization schema** - Company info
- **SoftwareApplication schema** - Tool info
- **HowTo schema** - For tutorial pages
- **FAQPage schema** - For FAQ page

Example for FAQ page:
```html
<script type="application/ld+json">
{
  "@context": "https://schema.org",
  "@type": "FAQPage",
  "mainEntity": [...]
}
</script>
```

### 6.5 Image Optimization

- Add `og:image` for each major page
- Create a proper social share image (1200x630px)
- Add alt text to all images

### 6.6 Internal Linking

**Current:** Limited cross-linking between pages

**Recommendation:**
- Add "Related Pages" sections at bottom of each page
- Link to comparison page from installation
- Cross-link CLI commands to tutorials that use them

---

## 7. Competitor SEO Analysis

### Rojo (rojo.space)

**Strengths:**
- Clean homepage with clear value prop
- "Powering Top Games on Roblox" social proof
- Good keyword usage ("Professional Development Tools")

**Weaknesses:**
- Limited blog/content marketing
- No comparison pages

**Keywords They Own:**
- "rojo roblox"
- "roblox vs code sync"
- "roblox professional development"

### Argon (argon.wiki)

**Strengths:**
- Feature-focused headings
- Good documentation structure

**Weaknesses:**
- Less SEO-optimized than Rojo
- No comparison content

**Keywords They Target:**
- "argon roblox"
- "roblox two way sync"

### Opportunity Gap

Neither competitor has:
- Comparison/alternative pages
- AI/MCP-focused content
- Migration guides
- Tutorial content

**RbxSync can own these keywords with targeted content.**

---

## 8. Quick Wins (Do First)

1. **Update page titles** in VitePress config with keywords
2. **Add per-page meta descriptions** via frontmatter
3. **Create comparison page** (Rojo vs Argon vs RbxSync)
4. **Fix homepage** to be a landing page, not redirect
5. **Add sitemap.xml and robots.txt**
6. **Update H1 headings** to include keywords

---

## 9. 30-Day Action Plan

| Week | Task | Impact |
|------|------|--------|
| 1 | Update all page titles and meta descriptions | High |
| 1 | Create /comparison/ page | High |
| 2 | Create proper homepage landing page | High |
| 2 | Add sitemap.xml and robots.txt | Medium |
| 3 | Create /migrate-from-rojo/ guide | High |
| 3 | Add structured data to key pages | Medium |
| 4 | Create 2-3 tutorial articles | Medium |
| 4 | Improve internal linking | Low |

---

## 10. Measuring Success

### Key Metrics to Track

| Metric | Tool | Target (90 days) |
|--------|------|------------------|
| Organic traffic | Google Search Console | +50% |
| Keyword rankings for "rojo alternative" | Ahrefs/Semrush | Top 10 |
| Keyword rankings for "roblox vscode sync" | Ahrefs/Semrush | Top 20 |
| Click-through rate from search | Google Search Console | >3% |
| Pages indexed | Google Search Console | All pages |

### Setup Required

1. **Google Search Console** - Verify site ownership
2. **Google Analytics** - Track traffic sources
3. **Ahrefs/Semrush** (optional) - Keyword tracking

---

## Summary

RbxSync has strong differentiation (AI-native, true two-way sync, zero-config) but the current site doesn't surface these advantages for search engines. With targeted title/meta updates, a comparison page, and better technical SEO, RbxSync can capture developers searching for Rojo alternatives and AI-powered Roblox development tools.

**Priority Actions:**
1. Fix page titles with keywords
2. Add per-page meta descriptions
3. Create comparison landing page
4. Fix homepage redirect
5. Add sitemap/robots.txt

---

*Report generated: 2026-01-16*
