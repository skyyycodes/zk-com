import { useEffect, useState } from 'react'
import { api } from './api'
import { ConsoleApp } from './console/ConsoleApp'
import { RegistryApp } from './registry/RegistryApp'

type Mode = 'registry' | 'console'

/** One app, two homes: the public registry (`zsolv serve`) or the treasurer's own console (`zsolv console`). */
export default function App() {
  const [mode, setMode] = useState<Mode | null>(null)

  useEffect(() => {
    // An older registry has no /api/mode; it is still a registry.
    api.mode().then(
      (m) => setMode(m.mode),
      () => setMode('registry'),
    )
  }, [])

  if (!mode) {
    return (
      <div className="empty" role="status">
        Loading…
      </div>
    )
  }
  return mode === 'console' ? <ConsoleApp /> : <RegistryApp />
}
