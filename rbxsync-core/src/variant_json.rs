//! Conversion between rbx_dom `Variant` property values and the `.rbxjson` `{ "type", "value" }` shape.
//!
//! `json_to_variant` and `variant_to_json` are inverses: `variant_to_json` must emit exactly
//! the shapes `json_to_variant` decodes. The round-trip tests below are the conformance guard.

use rbx_dom_weak::types::Variant;

/// Convert JSON property value to rbx_dom Variant
pub fn json_to_variant(value: &serde_json::Value) -> Option<Variant> {
    use rbx_dom_weak::types::*;

    // Check if it has a type field (our format)
    if let Some(obj) = value.as_object() {
        if let Some(type_str) = obj.get("type").and_then(|t| t.as_str()) {
            let val = obj.get("value");
            return match type_str {
                // Basic types
                "string" => val?.as_str().map(|s| Variant::String(s.to_string())),
                "int" | "int32" => val?.as_i64().map(|n| Variant::Int32(n as i32)),
                "int64" => val?.as_i64().map(Variant::Int64),
                "float" | "float32" => val?.as_f64().map(|n| Variant::Float32(n as f32)),
                "float64" | "double" => val?.as_f64().map(Variant::Float64),
                "bool" => val?.as_bool().map(Variant::Bool),

                // nil means "use default" - skip the property entirely
                "nil" => None,

                // Vector types
                "Vector2" => {
                    let v = val?.as_object()?;
                    Some(Variant::Vector2(Vector2::new(
                        v.get("x")?.as_f64()? as f32,
                        v.get("y")?.as_f64()? as f32,
                    )))
                }
                "Vector3" => {
                    let v = val?.as_object()?;
                    Some(Variant::Vector3(Vector3::new(
                        v.get("x")?.as_f64()? as f32,
                        v.get("y")?.as_f64()? as f32,
                        v.get("z")?.as_f64()? as f32,
                    )))
                }

                // Color types
                "Color3" => {
                    let v = val?.as_object()?;
                    Some(Variant::Color3(Color3::new(
                        v.get("r")?.as_f64()? as f32,
                        v.get("g")?.as_f64()? as f32,
                        v.get("b")?.as_f64()? as f32,
                    )))
                }
                "Color3uint8" => {
                    let v = val?.as_object()?;
                    Some(Variant::Color3uint8(Color3uint8::new(
                        v.get("r")?.as_u64()? as u8,
                        v.get("g")?.as_u64()? as u8,
                        v.get("b")?.as_u64()? as u8,
                    )))
                }
                "BrickColor" => {
                    val?.as_u64().map(|n| Variant::BrickColor(BrickColor::from_number(n as u16).unwrap_or(BrickColor::MediumStoneGrey)))
                }

                // UDim types
                "UDim" => {
                    let v = val?.as_object()?;
                    Some(Variant::UDim(UDim::new(
                        v.get("scale")?.as_f64()? as f32,
                        v.get("offset")?.as_i64()? as i32,
                    )))
                }
                "UDim2" => {
                    let v = val?.as_object()?;
                    let x = v.get("x")?.as_object()?;
                    let y = v.get("y")?.as_object()?;
                    Some(Variant::UDim2(UDim2::new(
                        UDim::new(
                            x.get("scale")?.as_f64()? as f32,
                            x.get("offset")?.as_i64()? as i32,
                        ),
                        UDim::new(
                            y.get("scale")?.as_f64()? as f32,
                            y.get("offset")?.as_i64()? as i32,
                        ),
                    )))
                }

                // CFrame
                "CFrame" => {
                    let v = val?.as_object()?;
                    let pos = v.get("position")?.as_array()?;
                    let rot = v.get("rotation")?.as_array()?;
                    if pos.len() >= 3 && rot.len() >= 9 {
                        Some(Variant::CFrame(CFrame::new(
                            Vector3::new(
                                pos[0].as_f64()? as f32,
                                pos[1].as_f64()? as f32,
                                pos[2].as_f64()? as f32,
                            ),
                            Matrix3::new(
                                Vector3::new(rot[0].as_f64()? as f32, rot[1].as_f64()? as f32, rot[2].as_f64()? as f32),
                                Vector3::new(rot[3].as_f64()? as f32, rot[4].as_f64()? as f32, rot[5].as_f64()? as f32),
                                Vector3::new(rot[6].as_f64()? as f32, rot[7].as_f64()? as f32, rot[8].as_f64()? as f32),
                            ),
                        )))
                    } else {
                        None
                    }
                }

                // Enum (store as u32)
                "Enum" => {
                    let v = val?.as_object()?;
                    let enum_value = v.get("value")?;
                    // Try to get numeric value, or parse from string
                    if let Some(n) = enum_value.as_u64() {
                        Some(Variant::Enum(rbx_dom_weak::types::Enum::from_u32(n as u32)))
                    } else {
                        // For string enum values, we'd need the reflection database
                        // For now, default to 0
                        Some(Variant::Enum(rbx_dom_weak::types::Enum::from_u32(0)))
                    }
                }

                // Rect
                "Rect" => {
                    let v = val?.as_object()?;
                    let min = v.get("min")?.as_object()?;
                    let max = v.get("max")?.as_object()?;
                    Some(Variant::Rect(Rect::new(
                        Vector2::new(min.get("x")?.as_f64()? as f32, min.get("y")?.as_f64()? as f32),
                        Vector2::new(max.get("x")?.as_f64()? as f32, max.get("y")?.as_f64()? as f32),
                    )))
                }

                // NumberRange
                "NumberRange" => {
                    let v = val?.as_object()?;
                    Some(Variant::NumberRange(NumberRange::new(
                        v.get("min")?.as_f64()? as f32,
                        v.get("max")?.as_f64()? as f32,
                    )))
                }

                // Font
                "Font" => {
                    let v = val?.as_object()?;
                    let family = v.get("family")?.as_str()?.to_string();
                    let weight = v.get("weight").and_then(|w| w.as_u64()).unwrap_or(400) as u16;
                    let style = v.get("style").and_then(|s| s.as_str()).unwrap_or("Normal");
                    Some(Variant::Font(Font {
                        family,
                        weight: FontWeight::from_u16(weight).unwrap_or(FontWeight::Regular),
                        style: if style == "Italic" { FontStyle::Italic } else { FontStyle::Normal },
                        cached_face_id: None,
                    }))
                }

                // Content (asset URLs)
                "Content" => {
                    val?.as_str().map(|s| Variant::Content(Content::from(s.to_string())))
                }

                // Refs - we skip these as they need special handling
                "Ref" => None,

                // Skip unknown/unsupported types
                _ => {
                    tracing::debug!("Unsupported property type: {}", type_str);
                    None
                }
            };
        }
    }

    // Direct value
    match value {
        serde_json::Value::String(s) => Some(Variant::String(s.clone())),
        serde_json::Value::Bool(b) => Some(Variant::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(Variant::Int32(i as i32))
            } else {
                n.as_f64().map(Variant::Float64)
            }
        }
        _ => None,
    }
}

/// Encode an rbx_dom Variant as the `.rbxjson` `{ "type", "value" }` shape.
/// Returns None for types we don't persist as context (binary/opaque, Ref).
pub fn variant_to_json(variant: &rbx_dom_weak::types::Variant) -> Option<serde_json::Value> {
    use serde_json::json;

    let out = match variant {
        Variant::String(s) => json!({ "type": "string", "value": s }),
        Variant::Int32(n) => json!({ "type": "int32", "value": n }),
        Variant::Int64(n) => json!({ "type": "int64", "value": n }),
        Variant::Float32(n) => json!({ "type": "float32", "value": n }),
        Variant::Float64(n) => json!({ "type": "float64", "value": n }),
        Variant::Bool(b) => json!({ "type": "bool", "value": b }),
        Variant::Vector2(v) => json!({ "type": "Vector2", "value": { "x": v.x, "y": v.y } }),
        Variant::Vector3(v) => json!({ "type": "Vector3", "value": { "x": v.x, "y": v.y, "z": v.z } }),
        Variant::Color3(c) => json!({ "type": "Color3", "value": { "r": c.r, "g": c.g, "b": c.b } }),
        Variant::Color3uint8(c) => json!({ "type": "Color3uint8", "value": { "r": c.r, "g": c.g, "b": c.b } }),
        Variant::BrickColor(bc) => json!({ "type": "BrickColor", "value": *bc as u16 }),
        Variant::UDim(u) => json!({ "type": "UDim", "value": { "scale": u.scale, "offset": u.offset } }),
        Variant::UDim2(u) => json!({ "type": "UDim2", "value": {
            "x": { "scale": u.x.scale, "offset": u.x.offset },
            "y": { "scale": u.y.scale, "offset": u.y.offset } } }),
        Variant::CFrame(cf) => json!({ "type": "CFrame", "value": {
            "position": [cf.position.x, cf.position.y, cf.position.z],
            "rotation": [
                cf.orientation.x.x, cf.orientation.x.y, cf.orientation.x.z,
                cf.orientation.y.x, cf.orientation.y.y, cf.orientation.y.z,
                cf.orientation.z.x, cf.orientation.z.y, cf.orientation.z.z ] } }),
        Variant::Enum(e) => json!({ "type": "Enum", "value": { "value": e.to_u32() } }),
        Variant::Rect(r) => json!({ "type": "Rect", "value": {
            "min": { "x": r.min.x, "y": r.min.y }, "max": { "x": r.max.x, "y": r.max.y } } }),
        Variant::NumberRange(nr) => json!({ "type": "NumberRange", "value": { "min": nr.min, "max": nr.max } }),
        Variant::Font(f) => json!({ "type": "Font", "value": {
            "family": f.family,
            "weight": f.weight.as_u16(),
            "style": match f.style { rbx_dom_weak::types::FontStyle::Italic => "Italic", _ => "Normal" } } }),
        Variant::ContentId(c) => json!({ "type": "Content", "value": c.as_str() }),
        Variant::Content(c) => match c.as_uri() {
            Some(uri) => json!({ "type": "Content", "value": uri }),
            None => return None,
        },
        _ => return None,
    };
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rbx_dom_weak::types::*;

    fn roundtrip(v: Variant) {
        let json = variant_to_json(&v).expect("should encode");
        let back = json_to_variant(&json).expect("should decode");
        // Compare via re-encode to avoid float PartialEq pitfalls on nested types
        assert_eq!(variant_to_json(&back), Some(json));
    }

    #[test]
    fn test_scalar_roundtrips() {
        roundtrip(Variant::String("hi".into()));
        roundtrip(Variant::Int32(7));
        roundtrip(Variant::Int64(9));
        roundtrip(Variant::Float32(1.5));
        roundtrip(Variant::Float64(2.25));
        roundtrip(Variant::Bool(true));
    }

    #[test]
    fn test_compound_roundtrips() {
        roundtrip(Variant::Vector2(Vector2::new(1.0, 2.0)));
        roundtrip(Variant::Vector3(Vector3::new(1.0, 2.0, 3.0)));
        roundtrip(Variant::Color3(Color3::new(0.1, 0.2, 0.3)));
        roundtrip(Variant::Color3uint8(Color3uint8::new(10, 20, 30)));
        roundtrip(Variant::UDim(UDim::new(0.5, 4)));
        roundtrip(Variant::UDim2(UDim2::new(UDim::new(0.5, 4), UDim::new(0.25, 8))));
        roundtrip(Variant::Rect(Rect::new(Vector2::new(0.0, 0.0), Vector2::new(5.0, 6.0))));
        roundtrip(Variant::NumberRange(NumberRange::new(1.0, 4.0)));
    }

    #[test]
    fn test_cframe_roundtrip() {
        roundtrip(Variant::CFrame(CFrame::new(
            Vector3::new(1.0, 2.0, 3.0),
            Matrix3::new(
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
            ),
        )));
    }

    #[test]
    fn test_enum_font_content_brickcolor_roundtrip() {
        roundtrip(Variant::Enum(Enum::from_u32(3)));
        roundtrip(Variant::BrickColor(BrickColor::from_number(1).unwrap()));
        roundtrip(Variant::Content(Content::from("rbxassetid://123".to_string())));
        roundtrip(Variant::Font(Font {
            family: "rbxasset://fonts/families/SourceSansPro.json".to_string(),
            weight: FontWeight::Bold,
            style: FontStyle::Italic,
            cached_face_id: None,
        }));
    }

    #[test]
    fn test_unsupported_returns_none() {
        assert_eq!(variant_to_json(&Variant::SharedString(
            SharedString::new(vec![1, 2, 3]))), None);
    }
}
