// Nav scroll effect
const nav = document.getElementById('main-nav');
window.addEventListener('scroll', () => {
  nav.classList.toggle('scrolled', window.scrollY > 16);
}, { passive: true });

// Hamburger menu
const hamburger = document.getElementById('hamburger');
const navDrawer = document.getElementById('nav-drawer');

hamburger.addEventListener('click', () => {
  const open = navDrawer.classList.toggle('open');
  hamburger.classList.toggle('open', open);
  hamburger.setAttribute('aria-expanded', open);
});

navDrawer.querySelectorAll('a').forEach(a => {
  a.addEventListener('click', () => {
    navDrawer.classList.remove('open');
    hamburger.classList.remove('open');
    hamburger.setAttribute('aria-expanded', 'false');
  });
});

// Scroll-triggered fade-in animation
const observer = new IntersectionObserver((entries) => {
  entries.forEach((entry) => {
    if (entry.isIntersecting) {
      entry.target.classList.add('visible');
    }
  });
}, { threshold: 0.1, rootMargin: '0px 0px -40px 0px' });

// Animate all major elements
const animateTargets = [
  '.pain-card', '.feature-item', '.pane-card', '.platform-card',
  '.problem h2', '.features h2', '.how h2', '.download h2',
  '.section-label', '.section-sub', '.hero-screenshot-wrapper'
];

animateTargets.forEach(selector => {
  document.querySelectorAll(selector).forEach((el, i) => {
    el.classList.add('fade-in');
    el.style.transitionDelay = `${i * 0.05}s`;
    observer.observe(el);
  });
});

// Populate download buttons with links to the latest GitHub release
const REPO = 'ire-research/ire';
const RELEASES_URL = `https://github.com/${REPO}/releases/latest`;

fetch(`https://api.github.com/repos/${REPO}/releases/latest`)
  .then((res) => res.json())
  .then((release) => {
    const assets = release.assets || [];
    const findAsset = (suffix) => assets.find((a) => a.name.endsWith(suffix));

    const links = {
      'download-dmg': findAsset('.dmg'),
      'download-exe': findAsset('.exe'),
      'download-appimage': findAsset('.AppImage'),
      'download-deb': findAsset('.deb'),
      'download-rpm': findAsset('.rpm'),
    };

    Object.entries(links).forEach(([id, asset]) => {
      const btn = document.getElementById(id);
      if (!btn) return;
      btn.href = asset ? asset.browser_download_url : RELEASES_URL;
      if (asset) btn.removeAttribute('target');
    });
  })
  .catch(() => {
    // Fall back to the releases page if the API call fails
    ['download-dmg', 'download-exe', 'download-appimage', 'download-deb', 'download-rpm'].forEach((id) => {
      const btn = document.getElementById(id);
      if (btn) btn.href = RELEASES_URL;
    });
  });

// Copy brew command
function copyBrew() {
  const cmd = 'brew install --cask ire';
  navigator.clipboard.writeText(cmd).then(() => {
    const btn = document.getElementById('copy-brew');
    btn.classList.add('copied');
    btn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>`;
    setTimeout(() => {
      btn.classList.remove('copied');
      btn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>`;
    }, 2000);
  });
}
