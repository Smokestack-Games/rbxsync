/**
 * Hover Provider for rbxjson files
 *
 * Provides hover documentation for class names, properties, and enum values.
 */

import { Hover, MarkupKind } from 'vscode-languageserver';
import { TextDocument } from 'vscode-languageserver-textdocument';
import { Position } from 'vscode-languageserver-types';
import { APIDumpHandler } from './apiDump';
import { analyzeContext, parseDocument } from './documentAnalyzer';

export class HoverProvider {
  constructor(private apiDump: APIDumpHandler) {}

  /**
   * Get hover information for a position
   */
  getHover(document: TextDocument, position: Position): Hover | null {
    const context = analyzeContext(document, position);
    const parsed = parseDocument(document);

    switch (context.type) {
      case 'className':
        return this.getClassHover(context.prefix || parsed.className);

      case 'propertyKey':
        return this.getPropertyHover(parsed.className, context.propertyName || context.prefix);

      case 'enumType':
        return this.getEnumTypeHover(context.prefix);

      case 'enumValue':
        return this.getEnumValueHover(context.enumType, context.prefix);

      case 'typeField':
        return this.getTypeHover(context.prefix);

      default:
        return null;
    }
  }

  /**
   * Hover for class name
   */
  private getClassHover(className: string | null): Hover | null {
    if (!className) return null;

    const classInfo = this.apiDump.getClassInfo(className);
    if (!classInfo) return null;

    const chain = this.apiDump.getInheritanceChain(className);
    const props = this.apiDump.getSerializableProperties(className);
    const ownProps = props.filter(p => p.definedIn === className);

    let content = `## ${className}\n\n`;

    if (classInfo.Superclass) {
      content += `**Extends:** ${classInfo.Superclass}\n\n`;
    }

    if (chain.length > 1) {
      content += `**Inheritance:** ${chain.join(' → ')}\n\n`;
    }

    if (classInfo.Tags && classInfo.Tags.length > 0) {
      content += `**Tags:** ${classInfo.Tags.join(', ')}\n\n`;
    }

    content += `---\n\n`;
    content += `**Properties:** ${props.length} total (${ownProps.length} own)\n\n`;

    if (ownProps.length > 0) {
      content += `| Property | Type |\n|----------|------|\n`;
      for (const prop of ownProps.slice(0, 15)) {
        content += `| ${prop.name} | ${prop.valueType.Name} |\n`;
      }
      if (ownProps.length > 15) {
        content += `| ... | (${ownProps.length - 15} more) |\n`;
      }
    }

    return {
      contents: {
        kind: MarkupKind.Markdown,
        value: content,
      },
    };
  }

  /**
   * Hover for property name
   */
  private getPropertyHover(className: string | null, propertyName: string | null): Hover | null {
    if (!className || !propertyName) return null;

    const prop = this.apiDump.getPropertyInfo(className, propertyName);
    if (!prop) return null;

    let content = `## ${className}.${propertyName}\n\n`;
    content += `**Type:** \`${prop.valueType.Name}\`\n\n`;

    if (prop.valueType.Category === 'Enum') {
      // Show enum values
      const enumValues = this.apiDump.getEnumValues(prop.valueType.Name);
      if (enumValues.length > 0) {
        content += `**Enum Values:**\n`;
        for (const item of enumValues.slice(0, 10)) {
          content += `- \`${item.Name}\` (${item.Value})\n`;
        }
        if (enumValues.length > 10) {
          content += `- ... and ${enumValues.length - 10} more\n`;
        }
        content += '\n';
      }
    }

    if (prop.category) {
      content += `**Category:** ${prop.category}\n\n`;
    }

    if (prop.definedIn !== className) {
      content += `**Inherited from:** ${prop.definedIn}\n\n`;
    }

    if (prop.defaultValue) {
      content += `**Default:** \`${prop.defaultValue}\`\n\n`;
    }

    content += `---\n\n`;

    // Security info
    content += `**Security:** Read: ${prop.security.Read}, Write: ${prop.security.Write}\n`;

    // Serialization info
    if (prop.serialization.CanSave && prop.serialization.CanLoad) {
      content += `**Serialization:** Can save and load\n`;
    } else {
      content += `**Serialization:** Save: ${prop.serialization.CanSave}, Load: ${prop.serialization.CanLoad}\n`;
    }

    if (prop.tags.length > 0) {
      content += `**Tags:** ${prop.tags.join(', ')}\n`;
    }

    // JSON format example
    content += `\n---\n\n`;
    content += `**rbxjson format:**\n`;
    content += '```json\n';
    content += this.getPropertyExample(prop);
    content += '\n```\n';

    return {
      contents: {
        kind: MarkupKind.Markdown,
        value: content,
      },
    };
  }

  /**
   * Generate example JSON for a property
   */
  private getPropertyExample(prop: { name: string; valueType: { Name: string; Category: string } }): string {
    const type = prop.valueType.Name;
    const category = prop.valueType.Category;

    if (category === 'Enum') {
      return `"${prop.name}": { "type": "Enum", "value": { "enumType": "${type}", "value": "..." } }`;
    }

    switch (type) {
      case 'bool':
        return `"${prop.name}": { "type": "bool", "value": true }`;
      case 'int':
      case 'int64':
      case 'float':
      case 'double':
        return `"${prop.name}": { "type": "${type}", "value": 0 }`;
      case 'string':
        return `"${prop.name}": { "type": "string", "value": "" }`;
      case 'Vector3':
        return `"${prop.name}": { "type": "Vector3", "value": { "x": 0, "y": 0, "z": 0 } }`;
      case 'Vector2':
        return `"${prop.name}": { "type": "Vector2", "value": { "x": 0, "y": 0 } }`;
      case 'Color3':
        return `"${prop.name}": { "type": "Color3", "value": { "r": 1, "g": 1, "b": 1 } }`;
      case 'CFrame':
      case 'CoordinateFrame':
        return `"${prop.name}": { "type": "CFrame", "value": { "position": [0,0,0], "rotation": [1,0,0,0,1,0,0,0,1] } }`;
      case 'UDim2':
        return `"${prop.name}": { "type": "UDim2", "value": { "x": { "scale": 0, "offset": 0 }, "y": { "scale": 0, "offset": 0 } } }`;
      default:
        return `"${prop.name}": { "type": "${type}", "value": ... }`;
    }
  }

  /**
   * Hover for enum type
   */
  private getEnumTypeHover(enumName: string | null): Hover | null {
    if (!enumName) return null;

    const enumInfo = this.apiDump.getEnumInfo(enumName);
    if (!enumInfo) return null;

    let content = `## Enum.${enumName}\n\n`;
    content += `**Values (${enumInfo.Items.length}):**\n\n`;
    content += `| Name | Value |\n|------|-------|\n`;

    for (const item of enumInfo.Items.slice(0, 20)) {
      content += `| ${item.Name} | ${item.Value} |\n`;
    }

    if (enumInfo.Items.length > 20) {
      content += `| ... | (${enumInfo.Items.length - 20} more) |\n`;
    }

    return {
      contents: {
        kind: MarkupKind.Markdown,
        value: content,
      },
    };
  }

  /**
   * Hover for enum value
   */
  private getEnumValueHover(enumType: string | null, valueName: string | null): Hover | null {
    if (!enumType || !valueName) return null;

    const enumInfo = this.apiDump.getEnumInfo(enumType);
    if (!enumInfo) return null;

    const item = enumInfo.Items.find(i => i.Name === valueName);
    if (!item) return null;

    const content = `## Enum.${enumType}.${valueName}\n\n` +
      `**Value:** ${item.Value}\n`;

    return {
      contents: {
        kind: MarkupKind.Markdown,
        value: content,
      },
    };
  }

  /**
   * Hover for type field
   */
  private getTypeHover(typeName: string | null): Hover | null {
    if (!typeName) return null;

    const typeDescriptions: Record<string, string> = {
      bool: 'Boolean value (true or false)',
      int: '32-bit signed integer',
      int64: '64-bit signed integer',
      float: 'Single-precision floating point',
      double: 'Double-precision floating point',
      string: 'Text string',
      Content: 'Asset URL or path (rbxassetid://, rbxasset://, etc.)',
      Vector2: '2D vector with x, y components',
      Vector3: '3D vector with x, y, z components',
      CFrame: 'Coordinate frame (position + rotation matrix)',
      Color3: 'RGB color with r, g, b components (0-1 range)',
      Color3uint8: 'RGB color with r, g, b components (0-255 range)',
      BrickColor: 'Legacy color by numeric ID',
      UDim: 'UI dimension with scale and offset',
      UDim2: 'UI position/size with x and y UDim values',
      Enum: 'Enumeration value with enumType and value',
      Ref: 'Reference to another instance by UUID',
      NumberSequence: 'Sequence of number keypoints',
      ColorSequence: 'Sequence of color keypoints',
      NumberRange: 'Range with min and max values',
      Font: 'Font family, weight, and style',
      Faces: 'Set of faces (top, bottom, left, right, front, back)',
      Axes: 'Set of axes (x, y, z)',
      PhysicalProperties: 'Physical material properties',
    };

    const description = typeDescriptions[typeName];
    if (!description) return null;

    let content = `## Type: ${typeName}\n\n`;
    content += `${description}\n\n`;
    content += `---\n\n`;
    content += `**Example:**\n`;
    content += '```json\n';
    content += this.getTypeExample(typeName);
    content += '\n```\n';

    return {
      contents: {
        kind: MarkupKind.Markdown,
        value: content,
      },
    };
  }

  /**
   * Get example JSON for a type
   */
  private getTypeExample(typeName: string): string {
    switch (typeName) {
      case 'bool':
        return '{ "type": "bool", "value": true }';
      case 'int':
      case 'int64':
      case 'float':
      case 'double':
        return `{ "type": "${typeName}", "value": 0 }`;
      case 'string':
        return '{ "type": "string", "value": "text" }';
      case 'Vector3':
        return '{ "type": "Vector3", "value": { "x": 0, "y": 0, "z": 0 } }';
      case 'Vector2':
        return '{ "type": "Vector2", "value": { "x": 0, "y": 0 } }';
      case 'Color3':
        return '{ "type": "Color3", "value": { "r": 1, "g": 1, "b": 1 } }';
      case 'CFrame':
        return '{ "type": "CFrame", "value": { "position": [0,0,0], "rotation": [1,0,0, 0,1,0, 0,0,1] } }';
      case 'Enum':
        return '{ "type": "Enum", "value": { "enumType": "Material", "value": "Plastic" } }';
      case 'UDim2':
        return '{ "type": "UDim2", "value": { "x": { "scale": 0, "offset": 0 }, "y": { "scale": 0, "offset": 0 } } }';
      default:
        return `{ "type": "${typeName}", "value": ... }`;
    }
  }
}
