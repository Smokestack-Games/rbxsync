/**
 * TypeScript interfaces for Roblox API Dump
 * Source: https://raw.githubusercontent.com/MaximumADHD/Roblox-Client-Tracker/roblox/API-Dump.json
 */

export interface APIDump {
  Version: number;
  Classes: ClassInfo[];
  Enums: EnumInfo[];
}

export interface ClassInfo {
  Name: string;
  Superclass: string;
  Members: MemberInfo[];
  Tags?: string[];
  MemoryCategory?: string;
}

export interface MemberInfo {
  MemberType: 'Property' | 'Function' | 'Event' | 'Callback';
  Name: string;
  ValueType?: ValueType;
  Security?: SecurityInfo;
  Serialization?: SerializationInfo;
  Tags?: string[];
  Category?: string;
  Default?: string;
  ThreadSafety?: string;
}

export interface ValueType {
  Category: string;  // 'Primitive', 'DataType', 'Class', 'Enum', 'Group'
  Name: string;      // 'bool', 'Vector3', 'Part', 'Material', etc.
}

export interface SecurityInfo {
  Read: string;   // 'None', 'PluginSecurity', 'RobloxSecurity', etc.
  Write: string;
}

export interface SerializationInfo {
  CanSave: boolean;
  CanLoad: boolean;
}

export interface EnumInfo {
  Name: string;
  Items: EnumItem[];
}

export interface EnumItem {
  Name: string;
  Value: number;
}

// Processed class info with inheritance resolved
export interface ResolvedClassInfo {
  name: string;
  superclass: string | null;
  properties: Map<string, PropertyInfo>;
  tags: string[];
  inheritanceChain: string[];
}

export interface PropertyInfo {
  name: string;
  valueType: ValueType;
  security: SecurityInfo;
  serialization: SerializationInfo;
  tags: string[];
  category: string;
  defaultValue?: string;
  definedIn: string;  // Class where property is defined
}

// rbxjson property value types (from rbxsync-core)
export const PROPERTY_TYPES = [
  'bool',
  'int',
  'int64',
  'float',
  'double',
  'string',
  'Content',
  'ProtectedString',
  'Vector2',
  'Vector2int16',
  'Vector3',
  'Vector3int16',
  'CFrame',
  'OptionalCFrame',
  'Color3',
  'Color3uint8',
  'BrickColor',
  'UDim',
  'UDim2',
  'Rect',
  'NumberSequence',
  'ColorSequence',
  'NumberRange',
  'Enum',
  'Ref',
  'Font',
  'Faces',
  'Axes',
  'PhysicalProperties',
  'Ray',
  'Region3',
  'Region3int16',
  'UniqueId',
  'SecurityCapabilities',
  'BinaryString',
  'SharedString',
] as const;

export type PropertyType = typeof PROPERTY_TYPES[number];

// Mapping from API dump ValueType to rbxjson type
export const VALUE_TYPE_TO_RBXJSON: Record<string, PropertyType> = {
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
  'CoordinateFrame': 'CFrame',
  'OptionalCoordinateFrame': 'OptionalCFrame',
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
  'UniqueId': 'UniqueId',
  'SecurityCapabilities': 'SecurityCapabilities',
  'BinaryString': 'BinaryString',
  'SharedString': 'SharedString',
  'Content': 'Content',
  'ProtectedString': 'ProtectedString',
};
