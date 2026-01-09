/**
 * API Dump Handler
 *
 * Fetches, caches, and provides efficient lookup for Roblox API information.
 */

import * as https from 'https';
import * as fs from 'fs';
import * as path from 'path';
import {
  APIDump,
  ClassInfo,
  EnumInfo,
  EnumItem,
  MemberInfo,
  PropertyInfo,
  ResolvedClassInfo,
  VALUE_TYPE_TO_RBXJSON,
  PropertyType,
} from './apiDumpTypes';

const API_DUMP_URL = 'https://raw.githubusercontent.com/MaximumADHD/Roblox-Client-Tracker/roblox/API-Dump.json';
const CACHE_DIR = '.rbxsync';
const CACHE_FILE = 'api-dump.json';
const CACHE_MAX_AGE_MS = 24 * 60 * 60 * 1000; // 24 hours

export class APIDumpHandler {
  private dump: APIDump | null = null;
  private classMap: Map<string, ClassInfo> = new Map();
  private enumMap: Map<string, EnumInfo> = new Map();
  private resolvedClasses: Map<string, ResolvedClassInfo> = new Map();
  private loading: Promise<void> | null = null;

  /**
   * Initialize the API dump handler (fetch or load from cache)
   */
  async initialize(): Promise<void> {
    if (this.dump) return;
    if (this.loading) return this.loading;

    this.loading = this.loadAPIDump();
    await this.loading;
    this.loading = null;
  }

  /**
   * Load API dump from cache or fetch from GitHub
   */
  private async loadAPIDump(): Promise<void> {
    // Try cache first
    const cached = this.loadFromCache();
    if (cached) {
      this.dump = cached;
      this.buildLookupMaps();
      console.log(`[rbxjson LSP] Loaded API dump from cache (${this.classMap.size} classes)`);
      return;
    }

    // Fetch from GitHub
    try {
      const data = await this.fetchFromGitHub();
      this.dump = data;
      this.buildLookupMaps();
      this.saveToCache(data);
      console.log(`[rbxjson LSP] Fetched API dump (${this.classMap.size} classes, ${this.enumMap.size} enums)`);
    } catch (error) {
      console.error('[rbxjson LSP] Failed to fetch API dump:', error);
      // Try bundled fallback
      const fallback = this.loadBundledFallback();
      if (fallback) {
        this.dump = fallback;
        this.buildLookupMaps();
        console.log(`[rbxjson LSP] Using bundled fallback (${this.classMap.size} classes)`);
      }
    }
  }

  /**
   * Fetch API dump from GitHub
   */
  private fetchFromGitHub(): Promise<APIDump> {
    return new Promise((resolve, reject) => {
      https.get(API_DUMP_URL, (res) => {
        if (res.statusCode !== 200) {
          reject(new Error(`HTTP ${res.statusCode}`));
          return;
        }

        let data = '';
        res.on('data', chunk => data += chunk);
        res.on('end', () => {
          try {
            resolve(JSON.parse(data));
          } catch (e) {
            reject(e);
          }
        });
        res.on('error', reject);
      }).on('error', reject);
    });
  }

  /**
   * Load from disk cache
   */
  private loadFromCache(): APIDump | null {
    try {
      const homeDir = process.env.HOME || process.env.USERPROFILE || '';
      const cachePath = path.join(homeDir, CACHE_DIR, CACHE_FILE);

      if (!fs.existsSync(cachePath)) return null;

      const stats = fs.statSync(cachePath);
      const age = Date.now() - stats.mtimeMs;

      if (age > CACHE_MAX_AGE_MS) {
        console.log('[rbxjson LSP] Cache expired');
        return null;
      }

      const content = fs.readFileSync(cachePath, 'utf8');
      return JSON.parse(content);
    } catch {
      return null;
    }
  }

  /**
   * Save to disk cache
   */
  private saveToCache(data: APIDump): void {
    try {
      const homeDir = process.env.HOME || process.env.USERPROFILE || '';
      const cacheDir = path.join(homeDir, CACHE_DIR);
      const cachePath = path.join(cacheDir, CACHE_FILE);

      if (!fs.existsSync(cacheDir)) {
        fs.mkdirSync(cacheDir, { recursive: true });
      }

      fs.writeFileSync(cachePath, JSON.stringify(data));
    } catch (error) {
      console.warn('[rbxjson LSP] Failed to save cache:', error);
    }
  }

  /**
   * Load bundled fallback (if available)
   */
  private loadBundledFallback(): APIDump | null {
    try {
      const fallbackPath = path.join(__dirname, '..', 'data', 'api-dump-fallback.json');
      if (fs.existsSync(fallbackPath)) {
        const content = fs.readFileSync(fallbackPath, 'utf8');
        return JSON.parse(content);
      }
    } catch {
      // Fallback not available
    }
    return null;
  }

  /**
   * Build lookup maps for fast access
   */
  private buildLookupMaps(): void {
    if (!this.dump) return;

    this.classMap.clear();
    this.enumMap.clear();
    this.resolvedClasses.clear();

    for (const cls of this.dump.Classes) {
      this.classMap.set(cls.Name, cls);
    }

    for (const enumInfo of this.dump.Enums) {
      this.enumMap.set(enumInfo.Name, enumInfo);
    }
  }

  /**
   * Get all class names
   */
  getAllClassNames(): string[] {
    return Array.from(this.classMap.keys()).sort();
  }

  /**
   * Get all enum names
   */
  getAllEnumNames(): string[] {
    return Array.from(this.enumMap.keys()).sort();
  }

  /**
   * Check if a class exists
   */
  hasClass(className: string): boolean {
    return this.classMap.has(className);
  }

  /**
   * Get raw class info
   */
  getClassInfo(className: string): ClassInfo | undefined {
    return this.classMap.get(className);
  }

  /**
   * Get enum info
   */
  getEnumInfo(enumName: string): EnumInfo | undefined {
    return this.enumMap.get(enumName);
  }

  /**
   * Get enum values for an enum type
   */
  getEnumValues(enumName: string): EnumItem[] {
    return this.enumMap.get(enumName)?.Items || [];
  }

  /**
   * Get resolved class info with inheritance
   */
  getResolvedClass(className: string): ResolvedClassInfo | undefined {
    // Check cache
    if (this.resolvedClasses.has(className)) {
      return this.resolvedClasses.get(className);
    }

    const classInfo = this.classMap.get(className);
    if (!classInfo) return undefined;

    // Build inheritance chain
    const inheritanceChain: string[] = [];
    const allProperties = new Map<string, PropertyInfo>();

    let current: ClassInfo | undefined = classInfo;
    while (current) {
      inheritanceChain.push(current.Name);

      // Collect properties from this class
      for (const member of current.Members) {
        if (member.MemberType !== 'Property') continue;
        if (allProperties.has(member.Name)) continue; // Don't override child properties

        const propInfo = this.memberToPropertyInfo(member, current.Name);
        if (propInfo) {
          allProperties.set(member.Name, propInfo);
        }
      }

      // Move to superclass
      current = current.Superclass ? this.classMap.get(current.Superclass) : undefined;
    }

    const resolved: ResolvedClassInfo = {
      name: className,
      superclass: classInfo.Superclass || null,
      properties: allProperties,
      tags: classInfo.Tags || [],
      inheritanceChain,
    };

    this.resolvedClasses.set(className, resolved);
    return resolved;
  }

  /**
   * Convert API dump member to PropertyInfo
   */
  private memberToPropertyInfo(member: MemberInfo, definedIn: string): PropertyInfo | null {
    if (!member.ValueType) return null;

    return {
      name: member.Name,
      valueType: member.ValueType,
      security: member.Security || { Read: 'None', Write: 'None' },
      serialization: member.Serialization || { CanSave: true, CanLoad: true },
      tags: member.Tags || [],
      category: member.Category || 'Data',
      defaultValue: member.Default,
      definedIn,
    };
  }

  /**
   * Get all properties for a class (including inherited)
   */
  getPropertiesForClass(className: string): PropertyInfo[] {
    const resolved = this.getResolvedClass(className);
    if (!resolved) return [];
    return Array.from(resolved.properties.values());
  }

  /**
   * Get serializable properties for a class (only user-editable properties)
   */
  getSerializableProperties(className: string): PropertyInfo[] {
    return this.getPropertiesForClass(className).filter(prop => {
      // Skip hidden/not scriptable/read-only
      if (prop.tags.includes('Hidden')) return false;
      if (prop.tags.includes('NotScriptable')) return false;
      if (prop.tags.includes('ReadOnly')) return false;
      if (prop.tags.includes('Deprecated')) return false;

      // Skip high read security
      if (prop.security.Read !== 'None' && prop.security.Read !== 'PluginSecurity') {
        return false;
      }

      // Skip high write security (must be writable)
      if (prop.security.Write !== 'None' && prop.security.Write !== 'PluginSecurity') {
        return false;
      }

      // Skip common non-data properties
      const skipProps = ['Parent', 'ClassName', 'Archivable', 'DataCost', 'RobloxLocked', 'Name'];
      if (skipProps.includes(prop.name)) return false;

      return true;
    });
  }

  /**
   * Get property info for a specific property on a class
   */
  getPropertyInfo(className: string, propertyName: string): PropertyInfo | undefined {
    const resolved = this.getResolvedClass(className);
    return resolved?.properties.get(propertyName);
  }

  /**
   * Get expected rbxjson type for a property
   */
  getExpectedType(className: string, propertyName: string): PropertyType | 'Enum' | 'Ref' | undefined {
    const prop = this.getPropertyInfo(className, propertyName);
    if (!prop) return undefined;

    const category = prop.valueType.Category;
    const name = prop.valueType.Name;

    // Enum category
    if (category === 'Enum') {
      return 'Enum';
    }

    // Class category (instance references)
    if (category === 'Class') {
      return 'Ref';
    }

    // Look up in mapping
    return VALUE_TYPE_TO_RBXJSON[name];
  }

  /**
   * Get inheritance chain for a class
   */
  getInheritanceChain(className: string): string[] {
    const resolved = this.getResolvedClass(className);
    return resolved?.inheritanceChain || [];
  }

  /**
   * Check if class inherits from another
   */
  inheritsFrom(className: string, baseClass: string): boolean {
    const chain = this.getInheritanceChain(className);
    return chain.includes(baseClass);
  }
}

// Singleton instance
let instance: APIDumpHandler | null = null;

export function getAPIDumpHandler(): APIDumpHandler {
  if (!instance) {
    instance = new APIDumpHandler();
  }
  return instance;
}
