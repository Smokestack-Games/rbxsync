/**
 * Completion Provider for rbxjson files
 *
 * Provides context-aware completions for class names, properties, types, and enum values.
 */

import {
  CompletionItem,
  CompletionItemKind,
  InsertTextFormat,
  MarkupKind,
} from 'vscode-languageserver';
import { TextDocument } from 'vscode-languageserver-textdocument';
import { Position } from 'vscode-languageserver-types';
import { APIDumpHandler } from './apiDump';
import { DocumentContext, analyzeContext } from './documentAnalyzer';
import { PROPERTY_TYPES, PropertyType } from './apiDumpTypes';

export class CompletionProvider {
  constructor(private apiDump: APIDumpHandler) {}

  /**
   * Get completions for a position in a document
   */
  getCompletions(document: TextDocument, position: Position): CompletionItem[] {
    const context = analyzeContext(document, position);

    switch (context.type) {
      case 'rootKey':
        return this.getRootKeyCompletions(context);

      case 'className':
        return this.getClassNameCompletions(context);

      case 'propertyKey':
        return this.getPropertyKeyCompletions(context);

      case 'typeField':
        return this.getTypeCompletions(context);

      case 'enumType':
        return this.getEnumTypeCompletions(context);

      case 'enumValue':
        return this.getEnumValueCompletions(context);

      case 'valueField':
        return this.getValueFieldCompletions(context);

      case 'vectorField':
        return []; // No completions for numeric fields

      default:
        return [];
    }
  }

  /**
   * Completions for root-level keys
   */
  private getRootKeyCompletions(context: DocumentContext): CompletionItem[] {
    const completions: CompletionItem[] = [];

    // Standard keys
    const keys = [
      { key: 'className', detail: 'Roblox class name', snippet: '"className": "${1:Part}"', required: true },
      { key: 'name', detail: 'Instance name', snippet: '"name": "${1}"', required: true },
      { key: 'referenceId', detail: 'Unique identifier (UUID)', snippet: '"referenceId": "${1}"', required: false },
      { key: 'path', detail: 'Instance path in tree', snippet: '"path": "${1}"', required: false },
      { key: 'properties', detail: 'Instance properties', snippet: '"properties": {\n\t$0\n}', required: false },
      { key: 'attributes', detail: 'Custom attributes', snippet: '"attributes": {\n\t$0\n}', required: false },
      { key: 'tags', detail: 'CollectionService tags', snippet: '"tags": [$0]', required: false },
    ];

    for (const k of keys) {
      if (k.key.toLowerCase().startsWith(context.prefix.toLowerCase())) {
        completions.push({
          label: k.key,
          kind: CompletionItemKind.Property,
          detail: k.detail,
          insertText: k.snippet,
          insertTextFormat: InsertTextFormat.Snippet,
          sortText: k.required ? '0' + k.key : '1' + k.key,
        });
      }
    }

    // Add class template snippets at root level
    const commonClasses = ['Part', 'Script', 'LocalScript', 'ModuleScript', 'Folder', 'Model', 'Frame', 'TextLabel', 'TextButton', 'Sound', 'ScreenGui'];
    for (const className of commonClasses) {
      if (className.toLowerCase().startsWith(context.prefix.toLowerCase()) ||
          'class'.startsWith(context.prefix.toLowerCase())) {
        const template = this.generateFullTemplate(className);
        completions.push({
          label: `${className} template`,
          kind: CompletionItemKind.Snippet,
          detail: `Full rbxjson for ${className}`,
          documentation: {
            kind: MarkupKind.Markdown,
            value: `Creates a complete **${className}** rbxjson with common properties.`,
          },
          insertText: template,
          insertTextFormat: InsertTextFormat.Snippet,
          sortText: '00' + className,
        });
      }
    }

    return completions;
  }

  /**
   * Generate a complete rbxjson template for a class
   */
  private generateFullTemplate(className: string): string {
    const props = this.apiDump.getSerializableProperties(className);
    const commonProps = this.getCommonPropertiesForClass(className, props);

    let template = `{\n  "className": "${className}",\n  "name": "\${1:${className}}",\n  "properties": {\n`;

    const propLines: string[] = [];
    let tabStop = 2;

    // Limit to most important properties for the template
    const limitedProps = commonProps.slice(0, 8);

    for (const prop of limitedProps) {
      const expectedType = this.apiDump.getExpectedType(className, prop.name);
      const propTemplate = this.getPropertyValueTemplate(prop.name, expectedType, tabStop);
      propLines.push(`    ${propTemplate}`);
      tabStop += this.countTabStops(propTemplate);
    }

    template += propLines.join(',\n');
    template += '\n  }\n}';

    return template;
  }

  /**
   * Completions for className values
   */
  private getClassNameCompletions(context: DocumentContext): CompletionItem[] {
    const classNames = this.apiDump.getAllClassNames();

    return classNames
      .filter(name => name.toLowerCase().startsWith(context.prefix.toLowerCase()))
      .map(name => {
        const classInfo = this.apiDump.getClassInfo(name);
        const superclass = classInfo?.Superclass;

        return {
          label: name,
          kind: CompletionItemKind.Class,
          detail: superclass ? `extends ${superclass}` : undefined,
          documentation: {
            kind: MarkupKind.Markdown,
            value: this.getClassDocumentation(name),
          },
          insertText: name,
          filterText: name,
          sortText: this.getClassSortText(name),
        };
      });
  }

  /**
   * Get all serializable properties for a class, with priority ones first
   */
  private getCommonPropertiesForClass(className: string, props: Array<{ name: string; definedIn: string; valueType: { Name: string } }>): Array<{ name: string; definedIn: string; valueType: { Name: string } }> {
    // Priority properties by class type (these go first)
    const priorityByClass: Record<string, string[]> = {
      'Part': ['Anchored', 'CanCollide', 'Size', 'CFrame', 'Color', 'Material', 'Transparency', 'Shape'],
      'MeshPart': ['Anchored', 'CanCollide', 'Size', 'CFrame', 'Color', 'Material', 'Transparency', 'MeshId', 'TextureID'],
      'Script': ['Disabled'],
      'LocalScript': ['Disabled'],
      'ModuleScript': [],
      'Frame': ['Size', 'Position', 'AnchorPoint', 'BackgroundColor3', 'BackgroundTransparency', 'BorderSizePixel', 'Visible'],
      'TextLabel': ['Text', 'TextColor3', 'TextSize', 'Font', 'Size', 'Position', 'BackgroundTransparency'],
      'TextButton': ['Text', 'TextColor3', 'TextSize', 'Font', 'Size', 'Position', 'BackgroundColor3'],
      'ImageLabel': ['Image', 'Size', 'Position', 'BackgroundTransparency', 'ImageColor3'],
      'ImageButton': ['Image', 'Size', 'Position', 'BackgroundTransparency', 'ImageColor3'],
      'Sound': ['SoundId', 'Volume', 'Looped', 'PlayOnRemove'],
      'ScreenGui': ['ResetOnSpawn', 'IgnoreGuiInset', 'ZIndexBehavior'],
      'Folder': [],
      'Model': ['PrimaryPart'],
      'RemoteEvent': [],
      'RemoteFunction': [],
      'BindableEvent': [],
      'BindableFunction': [],
    };

    const priority = priorityByClass[className] || [];
    const result: typeof props = [];
    const seen = new Set<string>();

    // Add priority properties first
    for (const propName of priority) {
      const prop = props.find(p => p.name === propName);
      if (prop && !seen.has(prop.name)) {
        result.push(prop);
        seen.add(prop.name);
      }
    }

    // Add remaining properties (own class first, then inherited)
    for (const prop of props) {
      if (!seen.has(prop.name) && prop.definedIn === className) {
        result.push(prop);
        seen.add(prop.name);
      }
    }

    // Add inherited properties
    for (const prop of props) {
      if (!seen.has(prop.name)) {
        result.push(prop);
        seen.add(prop.name);
      }
    }

    return result;
  }

  /**
   * Generate property value template with tab stops
   */
  private getPropertyValueTemplate(propName: string, expectedType: PropertyType | 'Enum' | 'Ref' | undefined, startTabStop: number): string {
    if (!expectedType) {
      return `"${propName}": { "type": "\${${startTabStop}}", "value": \${${startTabStop + 1}} }`;
    }

    switch (expectedType) {
      case 'bool':
        return `"${propName}": { "type": "bool", "value": \${${startTabStop}|true,false|} }`;
      case 'int':
      case 'int64':
      case 'float':
      case 'double':
        return `"${propName}": { "type": "${expectedType}", "value": \${${startTabStop}:0} }`;
      case 'string':
      case 'Content':
        return `"${propName}": { "type": "${expectedType}", "value": "\${${startTabStop}}" }`;
      case 'Vector3':
        return `"${propName}": { "type": "Vector3", "value": { "x": \${${startTabStop}:0}, "y": \${${startTabStop + 1}:0}, "z": \${${startTabStop + 2}:0} } }`;
      case 'Vector2':
        return `"${propName}": { "type": "Vector2", "value": { "x": \${${startTabStop}:0}, "y": \${${startTabStop + 1}:0} } }`;
      case 'Color3':
        return `"${propName}": { "type": "Color3", "value": { "r": \${${startTabStop}:1}, "g": \${${startTabStop + 1}:1}, "b": \${${startTabStop + 2}:1} } }`;
      case 'CFrame':
        return `"${propName}": { "type": "CFrame", "value": { "position": [\${${startTabStop}:0}, \${${startTabStop + 1}:0}, \${${startTabStop + 2}:0}], "rotation": [1,0,0, 0,1,0, 0,0,1] } }`;
      case 'Enum':
        return `"${propName}": { "type": "Enum", "value": { "enumType": "\${${startTabStop}}", "value": "\${${startTabStop + 1}}" } }`;
      case 'Ref':
        return `"${propName}": { "type": "Ref", "value": null }`;
      case 'UDim2':
        return `"${propName}": { "type": "UDim2", "value": { "x": { "scale": \${${startTabStop}:0}, "offset": \${${startTabStop + 1}:0} }, "y": { "scale": \${${startTabStop + 2}:0}, "offset": \${${startTabStop + 3}:0} } } }`;
      case 'BrickColor':
        return `"${propName}": { "type": "BrickColor", "value": \${${startTabStop}:194} }`;
      default:
        return `"${propName}": { "type": "${expectedType}", "value": \${${startTabStop}} }`;
    }
  }

  /**
   * Count tab stops in a template string
   */
  private countTabStops(template: string): number {
    const matches = template.match(/\$\{(\d+)/g);
    if (!matches) return 0;
    const numbers = matches.map(m => parseInt(m.replace('${', '')));
    return Math.max(...numbers) - Math.min(...numbers) + 1;
  }

  /**
   * Completions for property names in properties object
   */
  private getPropertyKeyCompletions(context: DocumentContext): CompletionItem[] {
    if (!context.className) {
      return [];
    }

    const properties = this.apiDump.getSerializableProperties(context.className);

    return properties
      .filter(prop => prop.name.toLowerCase().startsWith(context.prefix.toLowerCase()))
      .map(prop => {
        const expectedType = this.apiDump.getExpectedType(context.className!, prop.name);

        return {
          label: prop.name,
          kind: CompletionItemKind.Property,
          detail: `${prop.valueType.Name}${prop.definedIn !== context.className ? ` (from ${prop.definedIn})` : ''}`,
          documentation: {
            kind: MarkupKind.Markdown,
            value: this.getPropertyDocumentation(prop),
          },
          insertText: this.getPropertyInsertText(prop.name, expectedType),
          insertTextFormat: InsertTextFormat.Snippet,
          sortText: prop.definedIn === context.className ? '0' + prop.name : '1' + prop.name,
        };
      });
  }

  /**
   * Completions for type field values
   */
  private getTypeCompletions(context: DocumentContext): CompletionItem[] {
    // If we know the property, suggest the correct type first
    let expectedType: PropertyType | 'Enum' | 'Ref' | undefined;
    if (context.className && context.propertyName) {
      expectedType = this.apiDump.getExpectedType(context.className, context.propertyName);
    }

    const types = [...PROPERTY_TYPES, 'Enum', 'Ref'] as const;

    return types
      .filter(type => type.toLowerCase().startsWith(context.prefix.toLowerCase()))
      .map(type => ({
        label: type,
        kind: CompletionItemKind.TypeParameter,
        detail: type === expectedType ? '(expected)' : undefined,
        insertText: type,
        sortText: type === expectedType ? '0' + type : '1' + type,
      }));
  }

  /**
   * Completions for enumType field
   */
  private getEnumTypeCompletions(context: DocumentContext): CompletionItem[] {
    // If we know the property, suggest only valid enum types
    let expectedEnumType: string | undefined;
    if (context.className && context.propertyName) {
      const prop = this.apiDump.getPropertyInfo(context.className, context.propertyName);
      if (prop && prop.valueType.Category === 'Enum') {
        expectedEnumType = prop.valueType.Name;
      }
    }

    const enumNames = this.apiDump.getAllEnumNames();

    return enumNames
      .filter(name => name.toLowerCase().startsWith(context.prefix.toLowerCase()))
      .map(name => ({
        label: name,
        kind: CompletionItemKind.Enum,
        detail: name === expectedEnumType ? '(expected)' : undefined,
        insertText: name,
        sortText: name === expectedEnumType ? '0' + name : '1' + name,
      }));
  }

  /**
   * Completions for enum value field
   */
  private getEnumValueCompletions(context: DocumentContext): CompletionItem[] {
    if (!context.enumType) {
      return [];
    }

    const enumValues = this.apiDump.getEnumValues(context.enumType);

    return enumValues
      .filter(item => item.Name.toLowerCase().startsWith(context.prefix.toLowerCase()))
      .map(item => ({
        label: item.Name,
        kind: CompletionItemKind.EnumMember,
        detail: `${context.enumType}.${item.Name} = ${item.Value}`,
        insertText: item.Name,
      }));
  }

  /**
   * Completions for value field structure
   */
  private getValueFieldCompletions(context: DocumentContext): CompletionItem[] {
    if (!context.propertyType) {
      return [];
    }

    // Provide structure templates based on type
    switch (context.propertyType) {
      case 'Vector3':
        return this.getStructFieldCompletions(['x', 'y', 'z']);
      case 'Vector2':
        return this.getStructFieldCompletions(['x', 'y']);
      case 'Color3':
        return this.getStructFieldCompletions(['r', 'g', 'b']);
      case 'UDim':
        return this.getStructFieldCompletions(['scale', 'offset']);
      case 'UDim2':
        return [
          {
            label: 'x',
            kind: CompletionItemKind.Field,
            insertText: '"x": { "scale": ${1:0}, "offset": ${2:0} }',
            insertTextFormat: InsertTextFormat.Snippet,
          },
          {
            label: 'y',
            kind: CompletionItemKind.Field,
            insertText: '"y": { "scale": ${1:0}, "offset": ${2:0} }',
            insertTextFormat: InsertTextFormat.Snippet,
          },
        ];
      case 'Enum':
        return [
          {
            label: 'enumType',
            kind: CompletionItemKind.Field,
            insertText: '"enumType": "${1}"',
            insertTextFormat: InsertTextFormat.Snippet,
          },
          {
            label: 'value',
            kind: CompletionItemKind.Field,
            insertText: '"value": "${1}"',
            insertTextFormat: InsertTextFormat.Snippet,
          },
        ];
      default:
        return [];
    }
  }

  /**
   * Helper for struct field completions
   */
  private getStructFieldCompletions(fields: string[]): CompletionItem[] {
    return fields.map((field, i) => ({
      label: field,
      kind: CompletionItemKind.Field,
      insertText: `"${field}": \${${i + 1}:0}`,
      insertTextFormat: InsertTextFormat.Snippet,
    }));
  }

  /**
   * Generate insert text for a property with snippet
   */
  private getPropertyInsertText(propName: string, expectedType?: PropertyType | 'Enum' | 'Ref'): string {
    if (!expectedType) {
      return `"${propName}": { "type": "\${1}", "value": \${2} }`;
    }

    switch (expectedType) {
      case 'bool':
        return `"${propName}": { "type": "bool", "value": \${1|true,false|} }`;

      case 'int':
      case 'int64':
      case 'float':
      case 'double':
        return `"${propName}": { "type": "${expectedType}", "value": \${1:0} }`;

      case 'string':
      case 'Content':
        return `"${propName}": { "type": "${expectedType}", "value": "\${1}" }`;

      case 'Vector3':
        return `"${propName}": { "type": "Vector3", "value": { "x": \${1:0}, "y": \${2:0}, "z": \${3:0} } }`;

      case 'Vector2':
        return `"${propName}": { "type": "Vector2", "value": { "x": \${1:0}, "y": \${2:0} } }`;

      case 'Color3':
        return `"${propName}": { "type": "Color3", "value": { "r": \${1:1}, "g": \${2:1}, "b": \${3:1} } }`;

      case 'CFrame':
        return `"${propName}": { "type": "CFrame", "value": { "position": [\${1:0}, \${2:0}, \${3:0}], "rotation": [1,0,0, 0,1,0, 0,0,1] } }`;

      case 'Enum':
        return `"${propName}": { "type": "Enum", "value": { "enumType": "\${1}", "value": "\${2}" } }`;

      case 'Ref':
        return `"${propName}": { "type": "Ref", "value": "\${1:null}" }`;

      case 'UDim2':
        return `"${propName}": { "type": "UDim2", "value": { "x": { "scale": \${1:0}, "offset": \${2:0} }, "y": { "scale": \${3:0}, "offset": \${4:0} } } }`;

      default:
        return `"${propName}": { "type": "${expectedType}", "value": \${1} }`;
    }
  }

  /**
   * Get sort text to prioritize common classes
   */
  private getClassSortText(className: string): string {
    const commonClasses = [
      'Part', 'Script', 'LocalScript', 'ModuleScript', 'Folder',
      'Model', 'Frame', 'TextLabel', 'TextButton', 'ImageLabel',
      'Sound', 'ScreenGui', 'BillboardGui', 'RemoteEvent', 'RemoteFunction',
      'BindableEvent', 'BindableFunction', 'StringValue', 'NumberValue', 'BoolValue',
    ];

    const index = commonClasses.indexOf(className);
    if (index >= 0) {
      return '0' + index.toString().padStart(2, '0') + className;
    }
    return '1' + className;
  }

  /**
   * Generate documentation for a class
   */
  private getClassDocumentation(className: string): string {
    const chain = this.apiDump.getInheritanceChain(className);
    const classInfo = this.apiDump.getClassInfo(className);

    let doc = `**${className}**\n\n`;

    if (chain.length > 1) {
      doc += `Inherits: ${chain.slice(1).join(' → ')}\n\n`;
    }

    if (classInfo?.Tags && classInfo.Tags.length > 0) {
      doc += `Tags: ${classInfo.Tags.join(', ')}\n\n`;
    }

    const props = this.apiDump.getSerializableProperties(className);
    const ownProps = props.filter(p => p.definedIn === className);
    if (ownProps.length > 0) {
      doc += `**Properties** (${ownProps.length} own, ${props.length} total)\n`;
      doc += ownProps.slice(0, 10).map(p => `- ${p.name}: ${p.valueType.Name}`).join('\n');
      if (ownProps.length > 10) {
        doc += `\n- ... and ${ownProps.length - 10} more`;
      }
    }

    return doc;
  }

  /**
   * Generate documentation for a property
   */
  private getPropertyDocumentation(prop: {
    name: string;
    valueType: { Name: string; Category: string };
    definedIn: string;
    category: string;
    defaultValue?: string;
    tags: string[];
  }): string {
    let doc = `**${prop.name}**: ${prop.valueType.Name}\n\n`;

    if (prop.category) {
      doc += `Category: ${prop.category}\n`;
    }

    if (prop.definedIn) {
      doc += `Defined in: ${prop.definedIn}\n`;
    }

    if (prop.defaultValue) {
      doc += `Default: ${prop.defaultValue}\n`;
    }

    if (prop.tags.length > 0) {
      doc += `Tags: ${prop.tags.join(', ')}\n`;
    }

    return doc;
  }
}
