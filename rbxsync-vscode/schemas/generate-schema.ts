/**
 * Generates JSON Schema for .rbxjson files from MaximumADHD API dump
 *
 * Run with: npx ts-node schemas/generate-schema.ts
 * Or: npm run generate-schema
 */

import * as fs from 'fs';
import * as path from 'path';

const API_DUMP_URL = 'https://raw.githubusercontent.com/MaximumADHD/Roblox-Client-Tracker/roblox/Full-API-Dump.json';

// === Type Definitions from API Dump ===

interface ApiDump {
  Version: number;
  Classes: ApiClass[];
  Enums: ApiEnum[];
}

interface ApiClass {
  Name: string;
  Superclass: string;
  Members: ApiMember[];
  Tags?: string[];
  MemoryCategory?: string;
}

interface ApiMember {
  MemberType: 'Property' | 'Function' | 'Event' | 'Callback';
  Name: string;
  ValueType?: ApiValueType;
  Tags?: string[];
  Security?: { Read: string; Write: string } | string;
  Serialization?: { CanLoad: boolean; CanSave: boolean };
}

interface ApiValueType {
  Category: 'Primitive' | 'Class' | 'DataType' | 'Enum' | 'Group';
  Name: string;
}

interface ApiEnum {
  Name: string;
  Items: ApiEnumItem[];
}

interface ApiEnumItem {
  Name: string;
  Value: number;
}

// === JSON Schema Types ===

type JsonSchema = {
  $schema?: string;
  $id?: string;
  title?: string;
  description?: string;
  type?: string | string[];
  properties?: Record<string, JsonSchema>;
  additionalProperties?: boolean | JsonSchema;
  required?: string[];
  items?: JsonSchema;
  oneOf?: JsonSchema[];
  anyOf?: JsonSchema[];
  allOf?: JsonSchema[];
  if?: JsonSchema;
  then?: JsonSchema;
  else?: JsonSchema;
  enum?: (string | number | boolean | null)[];
  const?: string | number | boolean | null;
  $ref?: string;
  $defs?: Record<string, JsonSchema>;
  minimum?: number;
  maximum?: number;
  minItems?: number;
  maxItems?: number;
  minLength?: number;
  maxLength?: number;
  pattern?: string;
  default?: unknown;
  markdownDescription?: string;
};

// === Property Type Mappings ===

// Maps Roblox API ValueType names to our rbxjson type names
const VALUE_TYPE_MAP: Record<string, string> = {
  // Primitives
  'bool': 'bool',
  'int': 'int',
  'int64': 'int64',
  'float': 'float',
  'double': 'double',
  'string': 'string',

  // DataTypes
  'Vector2': 'Vector2',
  'Vector2int16': 'Vector2int16',
  'Vector3': 'Vector3',
  'Vector3int16': 'Vector3int16',
  'CFrame': 'CFrame',
  'Color3': 'Color3',
  'Color3uint8': 'Color3uint8',
  'BrickColor': 'BrickColor',
  'UDim': 'UDim',
  'UDim2': 'UDim2',
  'Rect': 'Rect',
  'NumberSequence': 'NumberSequence',
  'ColorSequence': 'ColorSequence',
  'NumberRange': 'NumberRange',
  'Font': 'Font',
  'Faces': 'Faces',
  'Axes': 'Axes',
  'PhysicalProperties': 'PhysicalProperties',
  'Ray': 'Ray',
  'Region3': 'Region3',
  'Region3int16': 'Region3int16',
  'Content': 'Content',
  'BinaryString': 'BinaryString',
  'SharedString': 'SharedString',
  'ProtectedString': 'ProtectedString',
  'OptionalCoordinateFrame': 'OptionalCFrame',
  'UniqueId': 'UniqueId',
  'SecurityCapabilities': 'SecurityCapabilities',
};

// === Schema Definitions for Property Value Types ===

function getPropertyTypeSchemas(): Record<string, JsonSchema> {
  return {
    // Primitives
    bool: {
      type: 'object',
      properties: {
        type: { const: 'bool' },
        value: { type: 'boolean' }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    int: {
      type: 'object',
      properties: {
        type: { const: 'int' },
        value: { type: 'integer' }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    int64: {
      type: 'object',
      properties: {
        type: { const: 'int64' },
        value: { type: 'integer' }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    float: {
      type: 'object',
      properties: {
        type: { const: 'float' },
        value: { type: 'number' }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    double: {
      type: 'object',
      properties: {
        type: { const: 'double' },
        value: { type: 'number' }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    string: {
      type: 'object',
      properties: {
        type: { const: 'string' },
        value: { type: 'string' }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },

    // Vectors
    Vector2: {
      type: 'object',
      properties: {
        type: { const: 'Vector2' },
        value: {
          type: 'object',
          properties: {
            x: { type: 'number' },
            y: { type: 'number' }
          },
          required: ['x', 'y'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    Vector2int16: {
      type: 'object',
      properties: {
        type: { const: 'Vector2int16' },
        value: {
          type: 'object',
          properties: {
            x: { type: 'integer', minimum: -32768, maximum: 32767 },
            y: { type: 'integer', minimum: -32768, maximum: 32767 }
          },
          required: ['x', 'y'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    Vector3: {
      type: 'object',
      properties: {
        type: { const: 'Vector3' },
        value: {
          type: 'object',
          properties: {
            x: { type: 'number' },
            y: { type: 'number' },
            z: { type: 'number' }
          },
          required: ['x', 'y', 'z'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    Vector3int16: {
      type: 'object',
      properties: {
        type: { const: 'Vector3int16' },
        value: {
          type: 'object',
          properties: {
            x: { type: 'integer', minimum: -32768, maximum: 32767 },
            y: { type: 'integer', minimum: -32768, maximum: 32767 },
            z: { type: 'integer', minimum: -32768, maximum: 32767 }
          },
          required: ['x', 'y', 'z'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },

    // Transform
    CFrame: {
      type: 'object',
      properties: {
        type: { const: 'CFrame' },
        value: {
          type: 'object',
          properties: {
            position: {
              type: 'array',
              items: { type: 'number' },
              minItems: 3,
              maxItems: 3,
              description: '[x, y, z] position'
            },
            rotation: {
              type: 'array',
              items: { type: 'number' },
              minItems: 9,
              maxItems: 9,
              description: '3x3 rotation matrix (row-major)'
            }
          },
          required: ['position', 'rotation'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },

    // Colors
    Color3: {
      type: 'object',
      properties: {
        type: { const: 'Color3' },
        value: {
          type: 'object',
          properties: {
            r: { type: 'number', minimum: 0, maximum: 1, description: 'Red (0-1)' },
            g: { type: 'number', minimum: 0, maximum: 1, description: 'Green (0-1)' },
            b: { type: 'number', minimum: 0, maximum: 1, description: 'Blue (0-1)' }
          },
          required: ['r', 'g', 'b'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    Color3uint8: {
      type: 'object',
      properties: {
        type: { const: 'Color3uint8' },
        value: {
          type: 'object',
          properties: {
            r: { type: 'integer', minimum: 0, maximum: 255, description: 'Red (0-255)' },
            g: { type: 'integer', minimum: 0, maximum: 255, description: 'Green (0-255)' },
            b: { type: 'integer', minimum: 0, maximum: 255, description: 'Blue (0-255)' }
          },
          required: ['r', 'g', 'b'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    BrickColor: {
      type: 'object',
      properties: {
        type: { const: 'BrickColor' },
        value: { type: 'integer', description: 'BrickColor number' }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },

    // UI Types
    UDim: {
      type: 'object',
      properties: {
        type: { const: 'UDim' },
        value: {
          type: 'object',
          properties: {
            scale: { type: 'number', description: 'Scale (0-1 typically)' },
            offset: { type: 'integer', description: 'Offset in pixels' }
          },
          required: ['scale', 'offset'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    UDim2: {
      type: 'object',
      properties: {
        type: { const: 'UDim2' },
        value: {
          type: 'object',
          properties: {
            x: {
              type: 'object',
              properties: {
                scale: { type: 'number' },
                offset: { type: 'integer' }
              },
              required: ['scale', 'offset'],
              additionalProperties: false
            },
            y: {
              type: 'object',
              properties: {
                scale: { type: 'number' },
                offset: { type: 'integer' }
              },
              required: ['scale', 'offset'],
              additionalProperties: false
            }
          },
          required: ['x', 'y'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    Rect: {
      type: 'object',
      properties: {
        type: { const: 'Rect' },
        value: {
          type: 'object',
          properties: {
            min: {
              type: 'object',
              properties: { x: { type: 'number' }, y: { type: 'number' } },
              required: ['x', 'y']
            },
            max: {
              type: 'object',
              properties: { x: { type: 'number' }, y: { type: 'number' } },
              required: ['x', 'y']
            }
          },
          required: ['min', 'max'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },

    // Sequences
    NumberSequence: {
      type: 'object',
      properties: {
        type: { const: 'NumberSequence' },
        value: {
          type: 'object',
          properties: {
            keypoints: {
              type: 'array',
              items: {
                type: 'object',
                properties: {
                  time: { type: 'number', minimum: 0, maximum: 1 },
                  value: { type: 'number' },
                  envelope: { type: 'number' }
                },
                required: ['time', 'value', 'envelope']
              }
            }
          },
          required: ['keypoints'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    ColorSequence: {
      type: 'object',
      properties: {
        type: { const: 'ColorSequence' },
        value: {
          type: 'object',
          properties: {
            keypoints: {
              type: 'array',
              items: {
                type: 'object',
                properties: {
                  time: { type: 'number', minimum: 0, maximum: 1 },
                  color: {
                    type: 'object',
                    properties: {
                      r: { type: 'number', minimum: 0, maximum: 1 },
                      g: { type: 'number', minimum: 0, maximum: 1 },
                      b: { type: 'number', minimum: 0, maximum: 1 }
                    },
                    required: ['r', 'g', 'b']
                  }
                },
                required: ['time', 'color']
              }
            }
          },
          required: ['keypoints'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    NumberRange: {
      type: 'object',
      properties: {
        type: { const: 'NumberRange' },
        value: {
          type: 'object',
          properties: {
            min: { type: 'number' },
            max: { type: 'number' }
          },
          required: ['min', 'max'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },

    // Enum
    Enum: {
      type: 'object',
      properties: {
        type: { const: 'Enum' },
        value: {
          type: 'object',
          properties: {
            enumType: { type: 'string', description: 'The Enum type name (e.g., "Material")' },
            value: { type: 'string', description: 'The Enum item name (e.g., "Plastic")' }
          },
          required: ['enumType', 'value'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },

    // References
    Ref: {
      type: 'object',
      properties: {
        type: { const: 'Ref' },
        value: {
          type: ['string', 'null'],
          pattern: '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$',
          description: 'UUID reference to another instance, or null'
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    Content: {
      type: 'object',
      properties: {
        type: { const: 'Content' },
        value: { type: 'string', description: 'Asset URL or rbxassetid://' }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },

    // Binary types
    BinaryString: {
      type: 'object',
      properties: {
        type: { const: 'BinaryString' },
        value: { type: 'string', description: 'Base64-encoded binary data' }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    SharedString: {
      type: 'object',
      properties: {
        type: { const: 'SharedString' },
        value: {
          type: 'object',
          properties: {
            hash: { type: 'string' },
            file: { type: ['string', 'null'] }
          },
          required: ['hash'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    ProtectedString: {
      type: 'object',
      properties: {
        type: { const: 'ProtectedString' },
        value: { type: 'string' }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },

    // Font
    Font: {
      type: 'object',
      properties: {
        type: { const: 'Font' },
        value: {
          type: 'object',
          properties: {
            family: { type: 'string', description: 'Font family (e.g., "rbxasset://fonts/families/SourceSansPro.json")' },
            weight: { type: 'string', description: 'Font weight (e.g., "Regular", "Bold")' },
            style: { type: 'string', description: 'Font style (e.g., "Normal", "Italic")' }
          },
          required: ['family', 'weight', 'style'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },

    // Faces/Axes
    Faces: {
      type: 'object',
      properties: {
        type: { const: 'Faces' },
        value: {
          type: 'object',
          properties: {
            top: { type: 'boolean' },
            bottom: { type: 'boolean' },
            left: { type: 'boolean' },
            right: { type: 'boolean' },
            front: { type: 'boolean' },
            back: { type: 'boolean' }
          },
          required: ['top', 'bottom', 'left', 'right', 'front', 'back'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    Axes: {
      type: 'object',
      properties: {
        type: { const: 'Axes' },
        value: {
          type: 'object',
          properties: {
            x: { type: 'boolean' },
            y: { type: 'boolean' },
            z: { type: 'boolean' }
          },
          required: ['x', 'y', 'z'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },

    // Physics
    PhysicalProperties: {
      type: 'object',
      properties: {
        type: { const: 'PhysicalProperties' },
        value: {
          type: 'object',
          properties: {
            density: { type: 'number' },
            friction: { type: 'number' },
            elasticity: { type: 'number' },
            friction_weight: { type: 'number' },
            elasticity_weight: { type: 'number' }
          },
          required: ['density', 'friction', 'elasticity', 'friction_weight', 'elasticity_weight'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    Ray: {
      type: 'object',
      properties: {
        type: { const: 'Ray' },
        value: {
          type: 'object',
          properties: {
            origin: {
              type: 'object',
              properties: { x: { type: 'number' }, y: { type: 'number' }, z: { type: 'number' } },
              required: ['x', 'y', 'z']
            },
            direction: {
              type: 'object',
              properties: { x: { type: 'number' }, y: { type: 'number' }, z: { type: 'number' } },
              required: ['x', 'y', 'z']
            }
          },
          required: ['origin', 'direction'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    Region3: {
      type: 'object',
      properties: {
        type: { const: 'Region3' },
        value: {
          type: 'object',
          properties: {
            min: {
              type: 'object',
              properties: { x: { type: 'number' }, y: { type: 'number' }, z: { type: 'number' } },
              required: ['x', 'y', 'z']
            },
            max: {
              type: 'object',
              properties: { x: { type: 'number' }, y: { type: 'number' }, z: { type: 'number' } },
              required: ['x', 'y', 'z']
            }
          },
          required: ['min', 'max'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    Region3int16: {
      type: 'object',
      properties: {
        type: { const: 'Region3int16' },
        value: {
          type: 'object',
          properties: {
            min: {
              type: 'object',
              properties: {
                x: { type: 'integer', minimum: -32768, maximum: 32767 },
                y: { type: 'integer', minimum: -32768, maximum: 32767 },
                z: { type: 'integer', minimum: -32768, maximum: 32767 }
              },
              required: ['x', 'y', 'z']
            },
            max: {
              type: 'object',
              properties: {
                x: { type: 'integer', minimum: -32768, maximum: 32767 },
                y: { type: 'integer', minimum: -32768, maximum: 32767 },
                z: { type: 'integer', minimum: -32768, maximum: 32767 }
              },
              required: ['x', 'y', 'z']
            }
          },
          required: ['min', 'max'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },

    // Special types
    OptionalCFrame: {
      type: 'object',
      properties: {
        type: { const: 'OptionalCFrame' },
        value: {
          oneOf: [
            { type: 'null' },
            {
              type: 'object',
              properties: {
                position: { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3 },
                rotation: { type: 'array', items: { type: 'number' }, minItems: 9, maxItems: 9 }
              },
              required: ['position', 'rotation']
            }
          ]
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    UniqueId: {
      type: 'object',
      properties: {
        type: { const: 'UniqueId' },
        value: { type: 'string' }
      },
      required: ['type', 'value'],
      additionalProperties: false
    },
    SecurityCapabilities: {
      type: 'object',
      properties: {
        type: { const: 'SecurityCapabilities' },
        value: { type: 'integer' }
      },
      required: ['type', 'value'],
      additionalProperties: false
    }
  };
}

// === Main Schema Generator ===

async function fetchApiDump(): Promise<ApiDump> {
  console.log('Fetching API dump from MaximumADHD...');
  const response = await fetch(API_DUMP_URL);
  if (!response.ok) {
    throw new Error(`Failed to fetch API dump: ${response.statusText}`);
  }
  return response.json() as Promise<ApiDump>;
}

function buildInheritanceMap(classes: ApiClass[]): Map<string, string[]> {
  // Maps className -> list of all ancestor class names
  const classMap = new Map<string, ApiClass>();
  for (const cls of classes) {
    classMap.set(cls.Name, cls);
  }

  const inheritanceMap = new Map<string, string[]>();

  for (const cls of classes) {
    const ancestors: string[] = [];
    let current = cls.Superclass;
    while (current && current !== '<<<ROOT>>>') {
      ancestors.push(current);
      const parentClass = classMap.get(current);
      if (parentClass) {
        current = parentClass.Superclass;
      } else {
        break;
      }
    }
    inheritanceMap.set(cls.Name, ancestors);
  }

  return inheritanceMap;
}

function getSerializableProperties(cls: ApiClass): ApiMember[] {
  return cls.Members.filter(member => {
    if (member.MemberType !== 'Property') return false;

    // Skip non-serializable properties
    if (member.Serialization && !member.Serialization.CanSave) return false;

    // Skip deprecated/hidden properties
    if (member.Tags?.includes('Deprecated')) return false;
    if (member.Tags?.includes('Hidden')) return false;
    if (member.Tags?.includes('NotScriptable')) return false;

    return true;
  });
}

function mapValueTypeToSchema(valueType: ApiValueType, enums: Map<string, ApiEnum>): string | null {
  if (valueType.Category === 'Primitive') {
    return VALUE_TYPE_MAP[valueType.Name] || null;
  }

  if (valueType.Category === 'DataType') {
    return VALUE_TYPE_MAP[valueType.Name] || null;
  }

  if (valueType.Category === 'Enum') {
    // For enums, we use the generic Enum type
    return 'Enum';
  }

  if (valueType.Category === 'Class') {
    // Instance references use Ref type
    return 'Ref';
  }

  return null;
}

function generateEnumSchemas(enums: ApiEnum[]): Record<string, JsonSchema> {
  const enumSchemas: Record<string, JsonSchema> = {};

  for (const enumDef of enums) {
    const itemNames = enumDef.Items.map(item => item.Name);
    enumSchemas[enumDef.Name] = {
      type: 'object',
      properties: {
        type: { const: 'Enum' },
        value: {
          type: 'object',
          properties: {
            enumType: { const: enumDef.Name },
            value: {
              type: 'string',
              enum: itemNames,
              description: `Valid values for Enum.${enumDef.Name}`
            }
          },
          required: ['enumType', 'value'],
          additionalProperties: false
        }
      },
      required: ['type', 'value'],
      additionalProperties: false
    };
  }

  return enumSchemas;
}

function generateClassPropertySchema(
  cls: ApiClass,
  inheritanceMap: Map<string, string[]>,
  classMap: Map<string, ApiClass>,
  enumMap: Map<string, ApiEnum>,
  propertyTypeSchemas: Record<string, JsonSchema>,
  enumSchemas: Record<string, JsonSchema>
): JsonSchema {
  // Collect all properties including inherited ones
  const allProperties: Map<string, { member: ApiMember; fromClass: string }> = new Map();

  // Add own properties
  for (const member of getSerializableProperties(cls)) {
    allProperties.set(member.Name, { member, fromClass: cls.Name });
  }

  // Add inherited properties
  const ancestors = inheritanceMap.get(cls.Name) || [];
  for (const ancestorName of ancestors) {
    const ancestor = classMap.get(ancestorName);
    if (ancestor) {
      for (const member of getSerializableProperties(ancestor)) {
        if (!allProperties.has(member.Name)) {
          allProperties.set(member.Name, { member, fromClass: ancestorName });
        }
      }
    }
  }

  // Build property schemas
  const propertySchemas: Record<string, JsonSchema> = {};

  for (const [propName, { member, fromClass }] of allProperties) {
    if (!member.ValueType) continue;

    const rbxjsonType = mapValueTypeToSchema(member.ValueType, enumMap);
    if (!rbxjsonType) continue;

    // For enum properties, use the specific enum schema if available
    if (member.ValueType.Category === 'Enum' && enumSchemas[member.ValueType.Name]) {
      propertySchemas[propName] = {
        ...enumSchemas[member.ValueType.Name],
        description: `${propName} (from ${fromClass}) - Enum.${member.ValueType.Name}`
      };
    } else if (propertyTypeSchemas[rbxjsonType]) {
      propertySchemas[propName] = {
        ...propertyTypeSchemas[rbxjsonType],
        description: `${propName} (from ${fromClass}) - ${rbxjsonType}`
      };
    }
  }

  return {
    type: 'object',
    additionalProperties: {
      description: 'Custom or unrecognized property',
      oneOf: Object.values(propertyTypeSchemas)
    },
    properties: propertySchemas
  };
}

async function generateSchema(): Promise<void> {
  const apiDump = await fetchApiDump();
  console.log(`API Version: ${apiDump.Version}`);
  console.log(`Classes: ${apiDump.Classes.length}`);
  console.log(`Enums: ${apiDump.Enums.length}`);

  const propertyTypeSchemas = getPropertyTypeSchemas();
  const enumMap = new Map(apiDump.Enums.map(e => [e.Name, e]));
  const enumSchemas = generateEnumSchemas(apiDump.Enums);
  const classMap = new Map(apiDump.Classes.map(c => [c.Name, c]));
  const inheritanceMap = buildInheritanceMap(apiDump.Classes);

  // Include all classes
  const serializableClasses = apiDump.Classes;
  console.log(`Serializable classes: ${serializableClasses.length}`);

  // Generate if/then conditions for each class
  const classConditions: JsonSchema[] = serializableClasses.map(cls => {
    const propertySchema = generateClassPropertySchema(
      cls,
      inheritanceMap,
      classMap,
      enumMap,
      propertyTypeSchemas,
      enumSchemas
    );

    return {
      if: {
        properties: {
          className: { const: cls.Name }
        },
        required: ['className']
      },
      then: {
        properties: {
          properties: propertySchema
        }
      }
    };
  });

  // Build the main schema using if/then for class-specific properties
  const schema: JsonSchema = {
    $schema: 'https://json-schema.org/draft-07/schema#',
    $id: 'https://rbxsync.dev/schemas/rbxjson.schema.json',
    title: 'RbxSync Instance JSON',
    description: `JSON schema for .rbxjson files. Auto-generated from Roblox API version ${apiDump.Version}.`,
    type: 'object',
    required: ['className'],
    properties: {
      className: {
        type: 'string',
        description: 'The Roblox class name',
        enum: serializableClasses.map(c => c.Name).sort()
      },
      properties: {
        type: 'object',
        description: 'Instance properties'
      },
      attributes: {
        type: 'object',
        description: 'Instance attributes',
        additionalProperties: { $ref: '#/$defs/AnyPropertyValue' }
      },
      tags: {
        type: 'array',
        items: { type: 'string' },
        description: 'CollectionService tags'
      },
      children: {
        type: 'array',
        items: { $ref: '#' },
        description: 'Child instances'
      },
      reference_id: {
        type: 'string',
        pattern: '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$',
        description: 'UUID for internal references'
      },
      name: {
        type: 'string',
        description: 'Instance name (alternative to Name property)'
      }
    },
    allOf: classConditions,
    $defs: {
      ...propertyTypeSchemas,
      ...enumSchemas,
      AnyPropertyValue: {
        oneOf: Object.values(propertyTypeSchemas)
      }
    }
  };

  // Write the schema
  const outputPath = path.join(__dirname, 'rbxjson.schema.json');
  fs.writeFileSync(outputPath, JSON.stringify(schema, null, 2));
  console.log(`\nSchema written to: ${outputPath}`);

  // Write metadata
  const metadata = {
    generatedAt: new Date().toISOString(),
    apiVersion: apiDump.Version,
    classCount: serializableClasses.length,
    enumCount: apiDump.Enums.length,
    sourceUrl: API_DUMP_URL
  };
  const metadataPath = path.join(__dirname, 'schema-metadata.json');
  fs.writeFileSync(metadataPath, JSON.stringify(metadata, null, 2));
  console.log(`Metadata written to: ${metadataPath}`);
}

// Run if executed directly
generateSchema().catch(console.error);
