// Applique le theme choisi des le chargement, avant le premier affichage (evite un flash du theme par defaut).
try {
  const t = localStorage.getItem('voiceclone-theme');
  if (t) document.documentElement.dataset.theme = t;
} catch { /* stockage indisponible : theme par defaut */ }
