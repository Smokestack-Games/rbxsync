/**
 * Document Analyzer for rbxjson files
 *
 * Parses JSON documents and provides context information for completions/hover.
 * Uses native JSON parsing to avoid bundling issues.
 */

import { Position, Range } from 'vscode-languageserver';
import { TextDocument } from 'vscode-languageserver-textdocument';

export type ContextType =
  | 'root'           // At root level of document
  | 'rootKey'        // Typing a root-level key (className, name, properties, etc.)
  | 'className'      // Inside className value
  | 'propertyKey'    // Typing a property name in properties object
  | 'propertyValue'  // Inside a property value
  | 'typeField'      // Inside "type" field of a property
  | 'valueField'     // Inside "value" field of a property
  | 'enumType'       // Inside enumType field
  | 'enumValue'      // Inside enum value field
  | 'vectorField'    // Inside vector x/y/z fields
  | 'attributeKey'   // Typing an attribute name
  | 'attributeValue' // Inside an attribute value
  | 'tag'            // Inside tags array
  | 'unknown';

export interface DocumentContext {
  type: ContextType;
  className: string | null;
  propertyName: string | null;
  propertyType: string | null;
  enumType: string | null;
  path: string[];       // JSON path to current location
  prefix: string;       // Text typed so far (for filtering completions)
  range: Range;         // Range to replace with completion
}

export interface ParsedDocument {
  className: string | null;
  name: string | null;
  properties: Record<string, unknown>;
  attributes: Record<string, unknown>;
  tags: string[];
  parseErrors: Array<{ message: string; offset: number }>;
}

/**
 * Parse an rbxjson document
 */
export function parseDocument(document: TextDocument): ParsedDocument {
  const text = document.getText();
  const errors: Array<{ message: string; offset: number }> = [];

  let json: Record<string, unknown> = {};
  try {
    json = JSON.parse(text);
  } catch (e) {
    const error = e as Error;
    // Try to extract position from error message
    const match = error.message.match(/position (\d+)/);
    const offset = match ? parseInt(match[1]) : 0;
    errors.push({ message: error.message, offset });
  }

  return {
    className: (json?.className as string) ?? null,
    name: (json?.name as string) ?? null,
    properties: (json?.properties as Record<string, unknown>) ?? {},
    attributes: (json?.attributes as Record<string, unknown>) ?? {},
    tags: (json?.tags as string[]) ?? [],
    parseErrors: errors,
  };
}

/**
 * Analyze context at a position for completions
 */
export function analyzeContext(document: TextDocument, position: Position): DocumentContext {
  const text = document.getText();
  const offset = document.offsetAt(position);

  // Parse document to get className
  const parsed = parseDocument(document);

  // Calculate prefix (text before cursor on current token)
  const prefix = getPrefix(text, offset);

  // Calculate range to replace
  const range = getPrefixRange(document, position, prefix);

  // Default context
  const context: DocumentContext = {
    type: 'unknown',
    className: parsed.className,
    propertyName: null,
    propertyType: null,
    enumType: null,
    path: [],
    prefix,
    range,
  };

  // Analyze context based on text before cursor
  const textBefore = text.substring(0, offset);

  // Check if we're in a string value
  const inString = isInString(textBefore);

  // Find what key we're in by looking backwards
  const keyContext = findKeyContext(textBefore);

  if (!keyContext) {
    // At root level
    if (isAfterColon(textBefore)) {
      context.type = 'unknown';
    } else {
      context.type = 'rootKey';
    }
    return context;
  }

  const { key, parentKey, inValue, grandparentKey } = keyContext;

  // className value
  if (key === 'className' && inValue) {
    context.type = 'className';
    return context;
  }

  // Inside properties object
  if (key === 'properties' || parentKey === 'properties') {
    if (parentKey === 'properties') {
      // We're inside a specific property
      context.propertyName = key;

      // Get the property value to find its type
      const propValue = parsed.properties[key];
      if (propValue && typeof propValue === 'object') {
        context.propertyType = (propValue as Record<string, unknown>).type as string;

        // Check if we're in the type field
        const lastKey = findLastKey(textBefore);
        if (lastKey === 'type' && inValue) {
          context.type = 'typeField';
          return context;
        }

        // Check if we're in value field
        if (lastKey === 'value' && inValue) {
          context.type = 'valueField';
          return context;
        }

        // Check for enum fields
        if (lastKey === 'enumType' && inValue) {
          context.type = 'enumType';
          return context;
        }

        if (lastKey === 'value' && context.propertyType === 'Enum') {
          // Get enumType from the property
          const valueObj = (propValue as Record<string, unknown>).value;
          if (valueObj && typeof valueObj === 'object') {
            context.enumType = (valueObj as Record<string, unknown>).enumType as string;
          }
          context.type = 'enumValue';
          return context;
        }
      }

      context.type = 'propertyValue';
      return context;
    }

    // At property key level
    context.type = 'propertyKey';
    return context;
  }

  // Inside attributes
  if (key === 'attributes' || parentKey === 'attributes') {
    context.type = parentKey === 'attributes' ? 'attributeValue' : 'attributeKey';
    return context;
  }

  // Inside tags
  if (key === 'tags') {
    context.type = 'tag';
    return context;
  }

  // Root level key
  if (!parentKey && !inValue) {
    context.type = 'rootKey';
    return context;
  }

  return context;
}

/**
 * Check if position is inside a string
 */
function isInString(textBefore: string): boolean {
  let inString = false;
  let escaped = false;

  for (const char of textBefore) {
    if (escaped) {
      escaped = false;
      continue;
    }
    if (char === '\\') {
      escaped = true;
      continue;
    }
    if (char === '"') {
      inString = !inString;
    }
  }

  return inString;
}

/**
 * Check if cursor is after a colon (expecting value)
 */
function isAfterColon(textBefore: string): boolean {
  const trimmed = textBefore.trimEnd();
  const lastNonWhitespace = trimmed[trimmed.length - 1];
  return lastNonWhitespace === ':';
}

/**
 * Find the key context at cursor position
 */
function findKeyContext(textBefore: string): { key: string; parentKey: string | null; inValue: boolean; grandparentKey: string | null } | null {
  // Track brace/bracket depth and find keys
  const keys: string[] = [];
  let currentKey = '';
  let inString = false;
  let escaped = false;
  let depth = 0;
  let sawColonAtCurrentDepth = false;
  let currentlyInValueString = false;

  for (let i = 0; i < textBefore.length; i++) {
    const char = textBefore[i];

    if (escaped) {
      escaped = false;
      if (inString) currentKey += char;
      continue;
    }

    if (char === '\\') {
      escaped = true;
      if (inString) currentKey += char;
      continue;
    }

    if (char === '"') {
      if (inString) {
        // End of string - check if this was a key
        const afterString = textBefore.substring(i + 1).trimStart();
        if (afterString.startsWith(':')) {
          keys[depth] = currentKey;
          sawColonAtCurrentDepth = false;
        }
        currentKey = '';
        currentlyInValueString = false;
      } else {
        // Starting a string - is it a value (after colon) or a key?
        currentlyInValueString = sawColonAtCurrentDepth;
      }
      inString = !inString;
      continue;
    }

    if (inString) {
      currentKey += char;
      continue;
    }

    if (char === '{' || char === '[') {
      depth++;
      sawColonAtCurrentDepth = false;
    } else if (char === '}' || char === ']') {
      depth = Math.max(0, depth - 1);
      sawColonAtCurrentDepth = false;
    } else if (char === ':') {
      sawColonAtCurrentDepth = true;
    } else if (char === ',') {
      sawColonAtCurrentDepth = false;
    }
  }

  if (keys.length === 0) return null;

  const key = keys[keys.length - 1] || '';
  const parentKey = keys.length > 1 ? keys[keys.length - 2] : null;
  const grandparentKey = keys.length > 2 ? keys[keys.length - 3] : null;

  // We're in a value if we're currently inside a string that started after a colon
  const inValue = inString && currentlyInValueString;

  return { key, parentKey, inValue, grandparentKey };
}

/**
 * Find the last key before cursor
 */
function findLastKey(textBefore: string): string | null {
  // Find the last "key": pattern
  const matches = textBefore.match(/"([^"]+)"\s*:\s*[^,}]*$/);
  return matches ? matches[1] : null;
}

/**
 * Get the text prefix before cursor
 */
function getPrefix(text: string, offset: number): string {
  let start = offset;
  while (start > 0) {
    const char = text[start - 1];
    if (char === '"' || char === ':' || char === ',' || char === '{' || char === '[' || /\s/.test(char)) {
      break;
    }
    start--;
  }

  let prefix = text.slice(start, offset);
  if (prefix.startsWith('"')) {
    prefix = prefix.slice(1);
  }
  return prefix;
}

/**
 * Get range for prefix replacement
 */
function getPrefixRange(document: TextDocument, position: Position, prefix: string): Range {
  const startOffset = document.offsetAt(position) - prefix.length;
  const startPos = document.positionAt(startOffset);
  return {
    start: startPos,
    end: position,
  };
}

/**
 * Find all property names and their ranges in a document
 */
export function findPropertyRanges(document: TextDocument): Map<string, Range> {
  const text = document.getText();
  const ranges = new Map<string, Range>();

  // Simple regex to find property names in "properties": { "PropName": ... }
  const propsMatch = text.match(/"properties"\s*:\s*\{([^}]*(?:\{[^}]*\}[^}]*)*)\}/s);
  if (!propsMatch) return ranges;

  const propsContent = propsMatch[1];
  const propsStart = text.indexOf(propsMatch[0]) + propsMatch[0].indexOf(propsContent);

  // Find each property key
  const keyRegex = /"([^"]+)"\s*:/g;
  let match;
  while ((match = keyRegex.exec(propsContent)) !== null) {
    const propName = match[1];
    const keyStart = propsStart + match.index + 1; // +1 for opening quote
    const keyEnd = keyStart + propName.length;

    ranges.set(propName, {
      start: document.positionAt(keyStart),
      end: document.positionAt(keyEnd),
    });
  }

  return ranges;
}

/**
 * Get the range of the className value
 */
export function findClassNameRange(document: TextDocument): Range | null {
  const text = document.getText();
  const match = text.match(/"className"\s*:\s*"([^"]*)"/);

  if (!match) return null;

  const valueStart = text.indexOf(match[0]) + match[0].indexOf(match[1]);
  const valueEnd = valueStart + match[1].length;

  return {
    start: document.positionAt(valueStart),
    end: document.positionAt(valueEnd),
  };
}
