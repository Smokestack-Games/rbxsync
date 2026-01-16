/**
 * Rojo-compatible project.json generation for Luau LSP
 *
 * This module generates a `default.project.json` file that Luau LSP
 * can use to understand the project structure and provide proper
 * intellisense, type checking, and autocompletion.
 */

import * as fs from 'fs';
import * as path from 'path';

/** Rojo project tree node */
interface RojoTreeNode {
  $className?: string;
  $path?: string;
  [key: string]: RojoTreeNode | string | undefined;
}

/** Rojo project.json structure */
interface RojoProject {
  name: string;
  tree: RojoTreeNode;
  globIgnorePaths?: string[];
}

/** Known Roblox services that map to their own class names */
const KNOWN_SERVICES: Record<string, string> = {
  Workspace: 'Workspace',
  ServerScriptService: 'ServerScriptService',
  ServerStorage: 'ServerStorage',
  ReplicatedStorage: 'ReplicatedStorage',
  ReplicatedFirst: 'ReplicatedFirst',
  StarterGui: 'StarterGui',
  StarterPack: 'StarterPack',
  StarterPlayer: 'StarterPlayer',
  StarterPlayerScripts: 'StarterPlayerScripts',
  StarterCharacterScripts: 'StarterCharacterScripts',
  Players: 'Players',
  Lighting: 'Lighting',
  SoundService: 'SoundService',
  Chat: 'Chat',
  LocalizationService: 'LocalizationService',
  TestService: 'TestService',
  Teams: 'Teams',
  TextChatService: 'TextChatService',
  VoiceChatService: 'VoiceChatService',
};

/**
 * Generate a Rojo-compatible default.project.json for Luau LSP
 *
 * @param projectDir - Root directory of the rbxsync project
 * @param projectName - Name for the project (from rbxsync.json or fallback)
 * @returns Path to the generated file, or undefined if failed
 */
export async function generateProjectJson(
  projectDir: string,
  projectName?: string
): Promise<string | undefined> {
  const srcDir = path.join(projectDir, 'src');

  // Check if src directory exists
  if (!fs.existsSync(srcDir)) {
    console.log('[projectJson] No src directory found, skipping project.json generation');
    return undefined;
  }

  // Read rbxsync.json for project name if not provided
  if (!projectName) {
    const rbxsyncJsonPath = path.join(projectDir, 'rbxsync.json');
    if (fs.existsSync(rbxsyncJsonPath)) {
      try {
        const rbxsyncJson = JSON.parse(fs.readFileSync(rbxsyncJsonPath, 'utf-8'));
        projectName = rbxsyncJson.name;
      } catch (e) {
        // Ignore parsing errors
      }
    }
    projectName = projectName || path.basename(projectDir);
  }

  // Build the tree by scanning src directory
  const tree: RojoTreeNode = {
    $className: 'DataModel',
  };

  const entries = fs.readdirSync(srcDir, { withFileTypes: true });
  for (const entry of entries) {
    if (!entry.isDirectory()) continue;

    const serviceName = entry.name;
    const className = KNOWN_SERVICES[serviceName];

    if (className) {
      // Known service - use className
      tree[serviceName] = {
        $className: className,
        $path: `src/${serviceName}`,
      };
    } else {
      // Unknown folder - still include it with $path only
      tree[serviceName] = {
        $path: `src/${serviceName}`,
      };
    }
  }

  // Handle StarterPlayer special case - it has child services
  if (tree['StarterPlayer'] && typeof tree['StarterPlayer'] === 'object') {
    const starterPlayerDir = path.join(srcDir, 'StarterPlayer');
    if (fs.existsSync(starterPlayerDir)) {
      const starterPlayerNode = tree['StarterPlayer'] as RojoTreeNode;
      // Remove $path since we'll define children
      delete starterPlayerNode.$path;
      starterPlayerNode.$className = 'StarterPlayer';

      const spEntries = fs.readdirSync(starterPlayerDir, { withFileTypes: true });
      for (const spEntry of spEntries) {
        if (!spEntry.isDirectory()) continue;

        const childName = spEntry.name;
        const childClassName = KNOWN_SERVICES[childName];

        if (childClassName) {
          starterPlayerNode[childName] = {
            $className: childClassName,
            $path: `src/StarterPlayer/${childName}`,
          };
        } else {
          starterPlayerNode[childName] = {
            $path: `src/StarterPlayer/${childName}`,
          };
        }
      }
    }
  }

  const project: RojoProject = {
    name: projectName,
    tree,
    globIgnorePaths: ['**/node_modules'],
  };

  // Write the project file
  const projectJsonPath = path.join(projectDir, 'default.project.json');
  try {
    fs.writeFileSync(projectJsonPath, JSON.stringify(project, null, 2) + '\n');
    console.log(`[projectJson] Generated ${projectJsonPath}`);
    return projectJsonPath;
  } catch (e) {
    console.error('[projectJson] Failed to write project.json:', e);
    return undefined;
  }
}

/**
 * Check if a default.project.json already exists
 */
export function hasProjectJson(projectDir: string): boolean {
  return fs.existsSync(path.join(projectDir, 'default.project.json'));
}

/**
 * Add default.project.json to .gitignore if not already present
 *
 * The generated project.json is derived from the extracted files,
 * so it doesn't need to be tracked in version control.
 */
export async function addToGitignore(projectDir: string): Promise<void> {
  const gitignorePath = path.join(projectDir, '.gitignore');
  const entry = 'default.project.json';

  try {
    let content = '';
    if (fs.existsSync(gitignorePath)) {
      content = fs.readFileSync(gitignorePath, 'utf-8');
      // Check if already present
      if (content.includes(entry)) {
        return;
      }
    }

    // Add to gitignore
    const newContent = content.endsWith('\n') || content === ''
      ? content + entry + '\n'
      : content + '\n' + entry + '\n';

    fs.writeFileSync(gitignorePath, newContent);
    console.log('[projectJson] Added default.project.json to .gitignore');
  } catch (e) {
    // Ignore errors - not critical
    console.log('[projectJson] Could not update .gitignore:', e);
  }
}
