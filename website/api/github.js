// Vercel Serverless Function to proxy GitHub API with caching
// This avoids rate limiting issues for unauthenticated requests

export const config = {
  runtime: 'edge',
};

// In-memory cache (persists across warm invocations)
let cache = {
  repo: null,
  release: null,
  repoTime: 0,
  releaseTime: 0,
};

const CACHE_TTL = 5 * 60 * 1000; // 5 minutes

export default async function handler(request) {
  const url = new URL(request.url);
  const type = url.searchParams.get('type'); // 'repo' or 'release'

  const headers = {
    'Content-Type': 'application/json',
    'Access-Control-Allow-Origin': '*',
    'Cache-Control': 'public, s-maxage=300, stale-while-revalidate=600',
  };

  try {
    const now = Date.now();

    if (type === 'release') {
      // Check cache
      if (cache.release && now - cache.releaseTime < CACHE_TTL) {
        return new Response(JSON.stringify(cache.release), { headers });
      }

      const res = await fetch('https://api.github.com/repos/devmarissa/rbxsync/releases/latest', {
        headers: {
          'Accept': 'application/vnd.github.v3+json',
          'User-Agent': 'rbxsync-website',
        },
      });

      if (!res.ok) {
        // Return cached data if available, even if stale
        if (cache.release) {
          return new Response(JSON.stringify(cache.release), { headers });
        }
        throw new Error(`GitHub API error: ${res.status}`);
      }

      const data = await res.json();
      cache.release = data;
      cache.releaseTime = now;

      return new Response(JSON.stringify(data), { headers });
    } else {
      // Default: repo info (stars)
      if (cache.repo && now - cache.repoTime < CACHE_TTL) {
        return new Response(JSON.stringify(cache.repo), { headers });
      }

      const res = await fetch('https://api.github.com/repos/devmarissa/rbxsync', {
        headers: {
          'Accept': 'application/vnd.github.v3+json',
          'User-Agent': 'rbxsync-website',
        },
      });

      if (!res.ok) {
        if (cache.repo) {
          return new Response(JSON.stringify(cache.repo), { headers });
        }
        throw new Error(`GitHub API error: ${res.status}`);
      }

      const data = await res.json();
      cache.repo = data;
      cache.repoTime = now;

      return new Response(JSON.stringify(data), { headers });
    }
  } catch (error) {
    return new Response(
      JSON.stringify({ error: error.message }),
      { status: 500, headers }
    );
  }
}
