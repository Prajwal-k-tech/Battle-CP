import { MetadataRoute } from 'next'

export default function robots(): MetadataRoute.Robots {
  return {
    rules: {
      userAgent: '*',
      allow: '/',
      disallow: ['/game/', '/api/'], // Disallow crawling of specific game instances and API
    },
    sitemap: 'https://battle-cp.vercel.app/sitemap.xml',
  }
}
