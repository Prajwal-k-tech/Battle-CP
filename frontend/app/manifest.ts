import { MetadataRoute } from 'next'

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: 'Battle CP',
    short_name: 'BattleCP',
    description: 'Competitive Programming Naval Combat Game',
    start_url: '/',
    display: 'standalone',
    background_color: '#000000',
    theme_color: '#3b82f6',
    icons: [
      {
        src: '/favicon.ico',
        sizes: 'any',
        type: 'image/x-icon',
      },
    ],
  }
}
