# Property Types

Complete reference for all supported property types. Property values inside the
[context file](/file-formats/rbxjson) use the `{ "type": ..., "value": ... }`
encoding shown below.

## Primitive Types

### string
```json
"Name": { "type": "string", "value": "Hello" }
```

### bool
```json
"Anchored": { "type": "bool", "value": true }
```

### int / int32 / int64
```json
"Count": { "type": "int", "value": 42 }
```

### float / float32 / float64
```json
"Transparency": { "type": "float", "value": 0.5 }
```

## Vector Types

### Vector2
```json
"Position": {
  "type": "Vector2",
  "value": { "x": 100, "y": 200 }
}
```

### Vector3
```json
"Size": {
  "type": "Vector3",
  "value": { "x": 4, "y": 1, "z": 2 }
}
```

## Transform Types

### CFrame
```json
"CFrame": {
  "type": "CFrame",
  "value": {
    "position": [0, 10, 0],
    "rotation": [1, 0, 0, 0, 1, 0, 0, 0, 1]
  }
}
```

Rotation is a 3x3 matrix stored as 9 values (row-major).

## Color Types

### Color3
Floating-point RGB (0-1 range):
```json
"Color": {
  "type": "Color3",
  "value": { "r": 1, "g": 0.5, "b": 0 }
}
```

### Color3uint8
Integer RGB (0-255 range):
```json
"Color": {
  "type": "Color3uint8",
  "value": { "r": 255, "g": 128, "b": 0 }
}
```

### BrickColor
BrickColor number:
```json
"BrickColor": {
  "type": "BrickColor",
  "value": 194
}
```

## UI Types

### UDim
```json
"Width": {
  "type": "UDim",
  "value": { "scale": 0.5, "offset": 10 }
}
```

### UDim2
```json
"Size": {
  "type": "UDim2",
  "value": {
    "x": { "scale": 0.5, "offset": 0 },
    "y": { "scale": 1, "offset": -20 }
  }
}
```

### Rect
```json
"SliceCenter": {
  "type": "Rect",
  "value": {
    "min": { "x": 10, "y": 10 },
    "max": { "x": 90, "y": 90 }
  }
}
```

## Range Types

### NumberRange
```json
"Range": {
  "type": "NumberRange",
  "value": { "min": 0, "max": 100 }
}
```

### NumberSequence
```json
"Transparency": {
  "type": "NumberSequence",
  "value": [
    { "time": 0, "value": 0 },
    { "time": 0.5, "value": 1 },
    { "time": 1, "value": 0 }
  ]
}
```

### ColorSequence
```json
"Color": {
  "type": "ColorSequence",
  "value": [
    { "time": 0, "color": { "r": 1, "g": 0, "b": 0 } },
    { "time": 1, "color": { "r": 0, "g": 0, "b": 1 } }
  ]
}
```

## Enum Type

```json
"Material": {
  "type": "Enum",
  "value": { "enumType": "Material", "value": "Plastic" }
}
```

## Content Type

Asset URLs:
```json
"Image": {
  "type": "Content",
  "value": "rbxassetid://123456"
}
```

## Font Type

```json
"FontFace": {
  "type": "Font",
  "value": {
    "family": "rbxasset://fonts/families/GothamSSm.json",
    "weight": 400,
    "style": "Normal"
  }
}
```

## Summary Table

| Type | Example Value |
|------|---------------|
| `string` | `"Hello"` |
| `bool` | `true` / `false` |
| `int` / `int32` / `int64` | `42` |
| `float` / `float32` / `float64` | `3.14` |
| `Vector2` | `{ "x": 0, "y": 0 }` |
| `Vector3` | `{ "x": 0, "y": 0, "z": 0 }` |
| `CFrame` | `{ "position": [...], "rotation": [...] }` |
| `Color3` | `{ "r": 1, "g": 0.5, "b": 0 }` |
| `Color3uint8` | `{ "r": 255, "g": 128, "b": 0 }` |
| `BrickColor` | `194` |
| `UDim` | `{ "scale": 0.5, "offset": 10 }` |
| `UDim2` | `{ "x": {...}, "y": {...} }` |
| `Rect` | `{ "min": {...}, "max": {...} }` |
| `NumberRange` | `{ "min": 0, "max": 100 }` |
| `Enum` | `{ "enumType": "...", "value": "..." }` |
| `Content` | `"rbxassetid://123456"` |
| `Font` | `{ "family": "...", "weight": 400, "style": "..." }` |
