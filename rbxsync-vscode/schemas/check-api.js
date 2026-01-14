const https = require('https');

const url = 'https://raw.githubusercontent.com/MaximumADHD/Roblox-Client-Tracker/roblox/Full-API-Dump.json';

https.get(url, (res) => {
  const chunks = [];
  res.on('data', chunk => chunks.push(chunk));
  res.on('end', () => {
    const data = JSON.parse(Buffer.concat(chunks).toString());
    console.log('Total classes in API dump:', data.Classes.length);

    // Count by tag
    const notCreatable = data.Classes.filter(c => c.Tags && c.Tags.includes('NotCreatable')).length;
    const services = data.Classes.filter(c => c.Tags && c.Tags.includes('Service')).length;
    const deprecated = data.Classes.filter(c => c.Tags && c.Tags.includes('Deprecated')).length;

    console.log('NotCreatable:', notCreatable);
    console.log('Services:', services);
    console.log('Deprecated:', deprecated);

    // What we're including vs excluding
    const serializable = data.Classes.filter(cls => {
      const tags = cls.Tags || [];
      if (tags.includes('NotCreatable') && tags.includes('Service')) return true;
      if (tags.includes('NotCreatable')) return false;
      return true;
    });
    console.log('\nCurrently included:', serializable.length);
    console.log('Excluded (abstract bases):', data.Classes.length - serializable.length);

    // List some excluded classes
    const excluded = data.Classes.filter(cls => {
      const tags = cls.Tags || [];
      return tags.includes('NotCreatable') && !tags.includes('Service');
    });
    console.log('\nSample excluded classes:');
    console.log(excluded.slice(0, 15).map(c => c.Name).join(', '));
  });
});
