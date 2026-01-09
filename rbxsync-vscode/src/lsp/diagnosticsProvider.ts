/**
 * Diagnostics Provider for rbxjson files
 *
 * Validates documents and reports errors/warnings.
 */

import { Diagnostic, DiagnosticSeverity, Range } from 'vscode-languageserver';
import { TextDocument } from 'vscode-languageserver-textdocument';
import { APIDumpHandler } from './apiDump';
import { parseDocument, findPropertyRanges, findClassNameRange } from './documentAnalyzer';
import { PROPERTY_TYPES, PropertyType, VALUE_TYPE_TO_RBXJSON } from './apiDumpTypes';

export class DiagnosticsProvider {
  constructor(private apiDump: APIDumpHandler) {}

  /**
   * Validate a document and return diagnostics
   */
  validate(document: TextDocument): Diagnostic[] {
    const diagnostics: Diagnostic[] = [];
    const parsed = parseDocument(document);

    // Don't report anything if JSON is invalid (user is still typing)
    // This prevents annoying error sounds while editing
    if (parsed.parseErrors.length > 0) {
      return [];
    }

    // Validate className (only if present - don't nag about missing fields)
    if (parsed.className) {
      if (!this.apiDump.hasClass(parsed.className)) {
        const range = findClassNameRange(document);
        if (range) {
          diagnostics.push({
            severity: DiagnosticSeverity.Error,
            range,
            message: `Unknown class: "${parsed.className}"`,
            source: 'rbxjson',
          });
        }
      }
    }

    // Validate properties
    if (parsed.className && this.apiDump.hasClass(parsed.className)) {
      const propertyRanges = findPropertyRanges(document);

      for (const [propName, propValue] of Object.entries(parsed.properties)) {
        const range = propertyRanges.get(propName);
        if (!range) continue;

        // Check if property exists on class
        const propInfo = this.apiDump.getPropertyInfo(parsed.className, propName);
        if (!propInfo) {
          diagnostics.push({
            severity: DiagnosticSeverity.Error,
            range,
            message: `Property "${propName}" does not exist on ${parsed.className}`,
            source: 'rbxjson',
          });
          continue;
        }

        // Check if property is serializable
        if (!propInfo.serialization.CanSave) {
          diagnostics.push({
            severity: DiagnosticSeverity.Warning,
            range,
            message: `Property "${propName}" is not serializable`,
            source: 'rbxjson',
          });
        }

        // Check if property has correct type wrapper
        if (propValue && typeof propValue === 'object') {
          const wrapper = propValue as Record<string, unknown>;
          this.validatePropertyValue(propName, wrapper, propInfo, range, diagnostics);
        } else {
          diagnostics.push({
            severity: DiagnosticSeverity.Error,
            range,
            message: `Property "${propName}" must be an object with "type" and "value" fields`,
            source: 'rbxjson',
          });
        }
      }
    }

    return diagnostics;
  }

  /**
   * Validate property value structure
   */
  private validatePropertyValue(
    propName: string,
    wrapper: Record<string, unknown>,
    propInfo: { valueType: { Name: string; Category: string } },
    range: Range,
    diagnostics: Diagnostic[]
  ): void {
    // Check for type field
    if (!('type' in wrapper)) {
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range,
        message: `Property "${propName}" is missing "type" field`,
        source: 'rbxjson',
      });
      return;
    }

    const declaredType = wrapper.type as string;

    // Validate type is known
    if (!this.isValidType(declaredType)) {
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range,
        message: `Unknown type "${declaredType}" for property "${propName}"`,
        source: 'rbxjson',
      });
      return;
    }

    // Check for value field
    if (!('value' in wrapper)) {
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range,
        message: `Property "${propName}" is missing "value" field`,
        source: 'rbxjson',
      });
      return;
    }

    // Validate type matches expected
    const expectedType = this.getExpectedRbxjsonType(propInfo.valueType);
    if (expectedType && declaredType !== expectedType) {
      // Special case: allow some type flexibility
      if (!this.areTypesCompatible(declaredType, expectedType)) {
        diagnostics.push({
          severity: DiagnosticSeverity.Warning,
          range,
          message: `Property "${propName}" has type "${declaredType}" but expected "${expectedType}"`,
          source: 'rbxjson',
        });
      }
    }

    // Validate value structure for complex types
    const value = wrapper.value;
    this.validateValueStructure(propName, declaredType, value, range, diagnostics);
  }

  /**
   * Validate value structure matches type
   */
  private validateValueStructure(
    propName: string,
    type: string,
    value: unknown,
    range: Range,
    diagnostics: Diagnostic[]
  ): void {
    switch (type) {
      case 'bool':
        if (typeof value !== 'boolean') {
          diagnostics.push({
            severity: DiagnosticSeverity.Error,
            range,
            message: `Property "${propName}" value must be a boolean`,
            source: 'rbxjson',
          });
        }
        break;

      case 'int':
      case 'int64':
      case 'float':
      case 'double':
        if (typeof value !== 'number') {
          diagnostics.push({
            severity: DiagnosticSeverity.Error,
            range,
            message: `Property "${propName}" value must be a number`,
            source: 'rbxjson',
          });
        }
        break;

      case 'string':
      case 'Content':
      case 'ProtectedString':
        if (typeof value !== 'string') {
          diagnostics.push({
            severity: DiagnosticSeverity.Error,
            range,
            message: `Property "${propName}" value must be a string`,
            source: 'rbxjson',
          });
        }
        break;

      case 'Vector2':
        this.validateVectorValue(propName, value, ['x', 'y'], range, diagnostics);
        break;

      case 'Vector3':
        this.validateVectorValue(propName, value, ['x', 'y', 'z'], range, diagnostics);
        break;

      case 'Color3':
        this.validateVectorValue(propName, value, ['r', 'g', 'b'], range, diagnostics);
        break;

      case 'Enum':
        this.validateEnumValue(propName, value, range, diagnostics);
        break;

      case 'CFrame':
        this.validateCFrameValue(propName, value, range, diagnostics);
        break;

      case 'UDim2':
        this.validateUDim2Value(propName, value, range, diagnostics);
        break;
    }
  }

  /**
   * Validate vector-like value (x,y,z or r,g,b)
   */
  private validateVectorValue(
    propName: string,
    value: unknown,
    fields: string[],
    range: Range,
    diagnostics: Diagnostic[]
  ): void {
    if (!value || typeof value !== 'object') {
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range,
        message: `Property "${propName}" value must be an object with ${fields.join(', ')} fields`,
        source: 'rbxjson',
      });
      return;
    }

    const obj = value as Record<string, unknown>;
    for (const field of fields) {
      if (!(field in obj)) {
        diagnostics.push({
          severity: DiagnosticSeverity.Error,
          range,
          message: `Property "${propName}" value is missing "${field}" field`,
          source: 'rbxjson',
        });
      } else if (typeof obj[field] !== 'number') {
        diagnostics.push({
          severity: DiagnosticSeverity.Error,
          range,
          message: `Property "${propName}" value.${field} must be a number`,
          source: 'rbxjson',
        });
      }
    }
  }

  /**
   * Validate enum value
   */
  private validateEnumValue(
    propName: string,
    value: unknown,
    range: Range,
    diagnostics: Diagnostic[]
  ): void {
    if (!value || typeof value !== 'object') {
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range,
        message: `Property "${propName}" Enum value must be an object with enumType and value fields`,
        source: 'rbxjson',
      });
      return;
    }

    const obj = value as Record<string, unknown>;

    if (!('enumType' in obj) || typeof obj.enumType !== 'string') {
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range,
        message: `Property "${propName}" Enum value is missing "enumType" field`,
        source: 'rbxjson',
      });
      return;
    }

    if (!('value' in obj) || typeof obj.value !== 'string') {
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range,
        message: `Property "${propName}" Enum value is missing "value" field`,
        source: 'rbxjson',
      });
      return;
    }

    // Validate enum type exists
    const enumType = obj.enumType as string;
    const enumInfo = this.apiDump.getEnumInfo(enumType);
    if (!enumInfo) {
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range,
        message: `Unknown enum type: "${enumType}"`,
        source: 'rbxjson',
      });
      return;
    }

    // Validate enum value exists
    const enumValue = obj.value as string;
    const validValues = enumInfo.Items.map(i => i.Name);
    if (!validValues.includes(enumValue)) {
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range,
        message: `Invalid enum value: "${enumValue}" is not a valid ${enumType} value`,
        source: 'rbxjson',
      });
    }
  }

  /**
   * Validate CFrame value
   */
  private validateCFrameValue(
    propName: string,
    value: unknown,
    range: Range,
    diagnostics: Diagnostic[]
  ): void {
    if (!value || typeof value !== 'object') {
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range,
        message: `Property "${propName}" CFrame value must be an object with position and rotation`,
        source: 'rbxjson',
      });
      return;
    }

    const obj = value as Record<string, unknown>;

    if (!('position' in obj) || !Array.isArray(obj.position) || obj.position.length !== 3) {
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range,
        message: `Property "${propName}" CFrame value.position must be an array of 3 numbers`,
        source: 'rbxjson',
      });
    }

    if (!('rotation' in obj) || !Array.isArray(obj.rotation) || obj.rotation.length !== 9) {
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range,
        message: `Property "${propName}" CFrame value.rotation must be an array of 9 numbers (3x3 rotation matrix)`,
        source: 'rbxjson',
      });
    }
  }

  /**
   * Validate UDim2 value
   */
  private validateUDim2Value(
    propName: string,
    value: unknown,
    range: Range,
    diagnostics: Diagnostic[]
  ): void {
    if (!value || typeof value !== 'object') {
      diagnostics.push({
        severity: DiagnosticSeverity.Error,
        range,
        message: `Property "${propName}" UDim2 value must be an object with x and y UDim values`,
        source: 'rbxjson',
      });
      return;
    }

    const obj = value as Record<string, unknown>;

    for (const axis of ['x', 'y']) {
      if (!(axis in obj)) {
        diagnostics.push({
          severity: DiagnosticSeverity.Error,
          range,
          message: `Property "${propName}" UDim2 value is missing "${axis}" field`,
          source: 'rbxjson',
        });
        continue;
      }

      const udim = obj[axis];
      if (!udim || typeof udim !== 'object') {
        diagnostics.push({
          severity: DiagnosticSeverity.Error,
          range,
          message: `Property "${propName}" UDim2 value.${axis} must be an object with scale and offset`,
          source: 'rbxjson',
        });
        continue;
      }

      const udimObj = udim as Record<string, unknown>;
      if (!('scale' in udimObj) || typeof udimObj.scale !== 'number') {
        diagnostics.push({
          severity: DiagnosticSeverity.Error,
          range,
          message: `Property "${propName}" UDim2 value.${axis}.scale must be a number`,
          source: 'rbxjson',
        });
      }
      if (!('offset' in udimObj) || typeof udimObj.offset !== 'number') {
        diagnostics.push({
          severity: DiagnosticSeverity.Error,
          range,
          message: `Property "${propName}" UDim2 value.${axis}.offset must be a number`,
          source: 'rbxjson',
        });
      }
    }
  }

  /**
   * Check if a type is valid
   */
  private isValidType(type: string): boolean {
    return PROPERTY_TYPES.includes(type as PropertyType) || type === 'Enum' || type === 'Ref';
  }

  /**
   * Get expected rbxjson type from API dump value type
   */
  private getExpectedRbxjsonType(valueType: { Name: string; Category: string }): string | undefined {
    if (valueType.Category === 'Enum') {
      return 'Enum';
    }
    if (valueType.Category === 'Class') {
      return 'Ref';
    }
    return VALUE_TYPE_TO_RBXJSON[valueType.Name];
  }

  /**
   * Check if two types are compatible
   */
  private areTypesCompatible(actual: string, expected: string): boolean {
    // Numeric types are compatible
    const numericTypes = ['int', 'int64', 'float', 'double'];
    if (numericTypes.includes(actual) && numericTypes.includes(expected)) {
      return true;
    }

    // String types are compatible
    const stringTypes = ['string', 'Content', 'ProtectedString'];
    if (stringTypes.includes(actual) && stringTypes.includes(expected)) {
      return true;
    }

    return actual === expected;
  }

  /**
   * Get human-readable parse error message
   */
  private getParseErrorMessage(error: number): string {
    const messages: Record<number, string> = {
      1: 'Invalid symbol',
      2: 'Invalid number format',
      3: 'Property name expected',
      4: 'Value expected',
      5: 'Colon expected',
      6: 'Comma expected',
      7: 'Closing brace expected',
      8: 'Closing bracket expected',
      9: 'End of file expected',
      10: 'Invalid comment token',
      11: 'Unexpected end of comment',
      12: 'Unexpected end of string',
      13: 'Unexpected end of number',
      14: 'Invalid unicode',
      15: 'Invalid escape character',
      16: 'Invalid character',
    };
    return messages[error] || 'JSON parse error';
  }
}
