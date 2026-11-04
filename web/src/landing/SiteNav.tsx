import { useEffect, useState } from 'react'
import type { RegistryState } from '../types'
import { Icon } from './kit'

/** Links into the landing page jump to a section; elsewhere they go home first. */
export const LINKS = [
  { name: 'The proof', href: '#/?to=proof' },
  { name: 'How it works', href: '#/?to=how' },
  { name: 'Privacy', href: '#/?to=privacy' },
  { name: 'Verify a link', href: '#/verify' },
]

/** Smooth-scroll to a landing section when it is already on the page. */
export function jump(e: React.MouseEvent<HTMLAnchorElement>, href: string) {
  const id = href.match(/\?to=(\w+)/)?.[1]
  const el = id && document.getElementById(id)
  if (el) {
    e.preventDefault()
    el.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }
}

export function SiteNav({ state, error, route }: { state: RegistryState | null; error: string | null; route: string }) {
  const [scrolled, setScrolled] = useState(false)
  const [open, setOpen] = useState(false)
  const landing = route.split('?')[0] === '/'
  const floating = scrolled || !landing

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 20)
    onScroll()
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [])

  useEffect(() => {
    document.body.style.overflow = open ? 'hidden' : ''
    return () => {
      document.body.style.overflow = ''
    }
  }, [open])

  const current = (href: string) => (href === '#/verify' && (route === '/verify' || route.startsWith('/v/')) ? 'page' : undefined)

  return (
    <header className={`site-nav ${floating ? 'is-floating' : ''} ${open ? 'is-open' : ''}`}>
      <nav aria-label="Main">
        <a href="#/" className="logo" onClick={() => setOpen(false)}>
          <span className="word">Z-Solvency</span>
          <span className="tm">ZEC</span>
        </a>

        <div className="site-links">
          {LINKS.map((l) => (
            <a key={l.name} href={l.href} onClick={(e) => jump(e, l.href)} aria-current={current(l.href)}>
              {l.name}
            </a>
          ))}
        </div>

        <div className="site-cta">
          <span className={`chain ${error ? 'offline' : ''}`}>{error ? 'offline' : state ? `tip ${state.tip}` : '…'}</span>
          <a className="btn sm" href="#/create" aria-current={route === '/create' ? 'page' : undefined}>
            Create your link
          </a>
        </div>

        <button type="button" className="menu-btn" onClick={() => setOpen(!open)} aria-expanded={open} aria-label={open ? 'Close menu' : 'Open menu'}>
          <Icon name={open ? 'x' : 'menu'} size={24} />
        </button>
      </nav>

      <div className="mobile-menu" hidden={!open}>
        <div className="mobile-links">
          {LINKS.map((l, i) => (
            <a
              key={l.name}
              href={l.href}
              style={{ transitionDelay: `${i * 75}ms` }}
              onClick={(e) => {
                jump(e, l.href)
                setOpen(false)
              }}
            >
              {l.name}
            </a>
          ))}
        </div>
        <div className="mobile-ctas">
          <a className="btn quiet lg" href="#/verify" onClick={() => setOpen(false)}>
            Verify a link
          </a>
          <a className="btn lg" href="#/create" onClick={() => setOpen(false)}>
            Create your link
          </a>
        </div>
      </div>
    </header>
  )
}
