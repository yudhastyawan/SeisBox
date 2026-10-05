// SeisBox Docs — main.js

// Sticky navbar shadow on scroll
const navbar = document.getElementById('navbar');
window.addEventListener('scroll', () => {
  if (window.scrollY > 10) {
    navbar.style.boxShadow = '0 4px 24px rgba(0,0,0,0.4)';
  } else {
    navbar.style.boxShadow = 'none';
  }
});

// Animate feature cards on scroll into view
const cards = document.querySelectorAll('.feature-card, .doc-card, .step');
const observer = new IntersectionObserver((entries) => {
  entries.forEach((entry, i) => {
    if (entry.isIntersecting) {
      entry.target.style.opacity = '1';
      entry.target.style.transform = 'translateY(0)';
      observer.unobserve(entry.target);
    }
  });
}, { threshold: 0.1 });

cards.forEach((card, i) => {
  card.style.opacity = '0';
  card.style.transform = 'translateY(20px)';
  card.style.transition = `opacity 0.4s ease ${i * 0.05}s, transform 0.4s ease ${i * 0.05}s, border-color 0.2s, box-shadow 0.2s`;
  observer.observe(card);
});

// Panel item cycling in the hero mockup
const panelItems = document.querySelectorAll('.panel-item');
let current = 0;
if (panelItems.length > 0) {
  setInterval(() => {
    panelItems[current].classList.remove('active');
    current = (current + 1) % panelItems.length;
    panelItems[current].classList.add('active');
  }, 2000);
}
