const schema = require('./rbxjson.schema.json');

const testFile = {
  "className": "Part",
  "properties": {
    "Name": { "type": "string", "value": "TestPart" },
    "Anchored": { "type": "bool", "value": true },
    "Size": { "type": "Vector3", "value": { "x": 4, "y": 1, "z": 2 } }
  }
};

// Show what properties Part class should have
const partCondition = schema.allOf.find(c => c.if?.properties?.className?.const === 'Part');
if (partCondition) {
  const props = Object.keys(partCondition.then.properties.properties.properties || {});
  console.log('=== Part class properties (' + props.length + ' total) ===');
  console.log(props.slice(0, 30).join('\n'));
  console.log('...\n');
}

// Count total classes
console.log('=== Schema Stats ===');
console.log('Total classes in enum:', schema.properties.className.enum.length);
console.log('Conditional schemas:', schema.allOf.length);

// Check if Part is valid
console.log('\n=== className validation ===');
console.log('Part in enum:', schema.properties.className.enum.includes('Part'));
console.log('BasePart in enum:', schema.properties.className.enum.includes('BasePart'));
console.log('Instance in enum:', schema.properties.className.enum.includes('Instance'));
