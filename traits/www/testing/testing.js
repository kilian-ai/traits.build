// Testing page navigation
document.addEventListener('DOMContentLoaded', () => {
  const navButtons = document.querySelectorAll('.nav-btn');
  const sections = document.querySelectorAll('.test-section');

  navButtons.forEach(button => {
    button.addEventListener('click', () => {
      const targetSection = button.dataset.section;

      // Remove active class from all buttons and sections
      navButtons.forEach(btn => btn.classList.remove('active'));
      sections.forEach(section => section.classList.remove('active'));

      // Add active class to clicked button and corresponding section
      button.classList.add('active');
      document.getElementById(targetSection)?.classList.add('active');

      // Save selected section to localStorage
      localStorage.setItem('traits.testing.section', targetSection);
    });
  });

  // Restore last viewed section
  const lastSection = localStorage.getItem('traits.testing.section') || 'overview';
  const lastButton = document.querySelector(`[data-section="${lastSection}"]`);
  if (lastButton) {
    lastButton.click();
  }
});

// Keyboard navigation (arrow keys to switch sections)
document.addEventListener('keydown', (e) => {
  const navButtons = Array.from(document.querySelectorAll('.nav-btn'));
  const activeButton = document.querySelector('.nav-btn.active');
  const activeIndex = navButtons.indexOf(activeButton);

  if (e.key === 'ArrowRight' && activeIndex < navButtons.length - 1) {
    navButtons[activeIndex + 1].click();
  } else if (e.key === 'ArrowLeft' && activeIndex > 0) {
    navButtons[activeIndex - 1].click();
  }
});
