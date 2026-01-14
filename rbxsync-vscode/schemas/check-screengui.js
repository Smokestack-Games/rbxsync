const https = require('https');

https.get('https://raw.githubusercontent.com/MaximumADHD/Roblox-Client-Tracker/roblox/API-Dump.json', (res) => {
  let data = '';
  res.on('data', chunk => data += chunk);
  res.on('end', () => {
    const dump = JSON.parse(data);
    const screenGui = dump.Classes.find(c => c.Name === 'ScreenGui');
    if (screenGui) {
      const props = screenGui.Members.filter(m => m.MemberType === 'Property');
      console.log('ScreenGui own properties:');
      props.forEach(p => console.log('  -', p.Name));

      // Check for ClipToDeviceSafeArea
      const hasClip = props.some(p => p.Name === 'ClipToDeviceSafeArea');
      console.log('\nHas ClipToDeviceSafeArea:', hasClip);

      // Also check LayerCollector (parent)
      const layerCollector = dump.Classes.find(c => c.Name === 'LayerCollector');
      if (layerCollector) {
        const lcProps = layerCollector.Members.filter(m => m.MemberType === 'Property');
        console.log('\nLayerCollector (parent) properties:');
        lcProps.forEach(p => console.log('  -', p.Name));
      }
    }
  });
});
