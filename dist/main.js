const { invoke } = window.__TAURI__.core;

// Application state
let currentContentType = 'movie';
let selectedGenres = [];
let selectedProviders = [];
let selectedLanguage = null;
let currentPage = 1;
let totalPages = 1;
let currentFilters = {};
let allGenres = {};
let allResults = []; // Store all loaded results
let currentPageType = 'discover'; // 'discover', 'watched', or 'white-list'
let watchedItems = [];
let filteredWatchedItems = [];
let currentWatchedFilter = 'all'; // 'all', 'movie', 'tv'
let whiteListItems = [];
let filteredWhiteListItems = [];
let currentWhiteListFilter = 'all'; // 'all', 'movie', 'tv'
let searchResults = [];
let searchPage = 1;
let searchTotalPages = 1;
let discoverGeneration = 0;
let nameGeneration = 0;
let nameQuery = '';
let nameTotals = { movie: 0, tv: 0 };
let nameLoading = false;
let aiWorkspace = null;
let researchDirty = false;
let aiRefreshing = false;
let mcpConnection = null;
let contentGeneration = 0;
let currentSeasonTrackerId = null; // ID of TV show currently open in season tracker

// DOM elements
let settingsModal;
let apiKeyInput;
let genresContainer;
let languageContainer;
let providersContainer;
let resultsGrid;
let resultsHeader;
let resultsInfo;
let viewMoreContainer;
let loading;
let emptyState;
let errorState;
let errorMessage;
let watchedGrid;
let watchedStats;
let emptyWatched;
let whiteListGrid;
let whiteListStats;
let emptyWhiteList;
let searchInput;
let searchClearBtn;
let searchNameResults;
let searchNameLoading;
let searchNameEmpty;
let searchNameError;

// Splash screen functionality
function initializeSplashScreen() {
  const splashScreen = document.getElementById('splash-screen');
  const splashLogo = document.getElementById('splash-logo');
  const mainApp = document.getElementById('main-app');

  // Start logo animation immediately
  splashLogo.classList.add('spin');

  setTimeout(() => invoke('disable_always_on_top').catch(() => {}), 1000);

  // Wait for animation to complete + 200ms delay
  setTimeout(() => {
    // Fade out splash screen
    splashScreen.classList.add('fade-out');

    // After fade out completes, show main app
    setTimeout(() => {
      splashScreen.style.display = 'none';
      mainApp.style.display = 'flex';
      mainApp.classList.add('show');
      document.body.classList.add('app-loaded');
    }, 500); // Match CSS transition duration
  }, 1000); // 800ms animation + 200ms delay
}

// Initialize the application
document.addEventListener('DOMContentLoaded', async () => {
  initializeSplashScreen();
  initializeElements();
  setupEventListeners();
  setupNewFeatures();
  initializeYearRange();
  document.getElementById('sort-by').value = 'popularity.desc';
  await loadApiKey();
  await Promise.all([loadWatchedItems(), loadWhiteListItems(), refreshAi(true)]);
  if (apiKeyInput.value) {
    await Promise.all([loadGenres(), loadLanguages(), loadWatchProviders()]);
    performSearch();
    checkForNewSeasons();
  } else {
    for (const element of [genresContainer, languageContainer, providersContainer]) element.textContent = 'Set your TMDB key in Settings to load options.';
    document.getElementById('filter-status').textContent = 'Set your TMDB key in Settings to begin';
  }
});

function initializeElements() {
  settingsModal = document.getElementById('settings-modal');
  apiKeyInput = document.getElementById('api-key-input');
  genresContainer = document.getElementById('genres-container');
  languageContainer = document.getElementById('language-container');
  providersContainer = document.getElementById('providers-container');
  resultsGrid = document.getElementById('results-grid');
  resultsHeader = document.getElementById('results-header');
  resultsInfo = document.getElementById('results-info');


  viewMoreContainer = document.getElementById('view-more-container');
  loading = document.getElementById('loading');
  emptyState = document.getElementById('empty-state');
  errorState = document.getElementById('error-state');
  errorMessage = document.getElementById('error-message');
  watchedGrid = document.getElementById('watched-grid');
  watchedStats = document.getElementById('watched-stats');
  emptyWatched = document.getElementById('empty-watched');
  whiteListGrid = document.getElementById('white-list-grid');
  whiteListStats = document.getElementById('white-list-stats');
  emptyWhiteList = document.getElementById('empty-white-list');
  searchInput = document.getElementById('search-input');
  searchClearBtn = document.getElementById('search-clear-btn');
  searchNameResults = document.getElementById('search-name-results');
  searchNameLoading = document.getElementById('search-name-loading');
  searchNameEmpty = document.getElementById('search-name-empty');
  searchNameError = document.getElementById('search-name-error');
}

function setupEventListeners() {
  // Settings modal
  document.getElementById('settings-btn').addEventListener('click', openSettings);
  document.getElementById('close-settings').addEventListener('click', closeSettings);
  document.getElementById('cancel-settings').addEventListener('click', closeSettings);
  document.getElementById('save-settings').addEventListener('click', saveApiKey);

  // Movie details modal & integrated season tracker
  document.getElementById('close-movie-details').addEventListener('click', closeMovieDetails);
  document.getElementById('season-tracker-save').addEventListener('click', saveSeasonTracker);

  // Navigation
  document.getElementById('discover-nav').addEventListener('click', () => switchPage('discover'));
  document.getElementById('search-nav').addEventListener('click', () => switchPage('search'));
  document.getElementById('watched-nav').addEventListener('click', () => switchPage('watched'));
  document.getElementById('white-list-nav').addEventListener('click', () => switchPage('white-list'));
  document.getElementById('go-to-discover').addEventListener('click', () => switchPage('discover'));

  // Content type toggle
  document.querySelectorAll('.toggle-btn').forEach(btn => {
    btn.addEventListener('click', (e) => {
      const type = e.currentTarget.dataset.type;
      if (type && type !== currentContentType) {
        switchContentType(type);
      }
    });
  });

  // Watched filter toggle
  document.querySelectorAll('[data-filter]').forEach(btn => {
    btn.addEventListener('click', (e) => {
      const filter = e.currentTarget.dataset.filter;
      if (filter && filter !== currentWatchedFilter) {
        switchWatchedFilter(filter);
      }
    });
  });

  // Search functionality
  document.getElementById('search-btn').addEventListener('click', performSearch);

  // Search page functionality
  document.getElementById('search-name-btn').addEventListener('click', performNameSearch);
  searchInput.addEventListener('input', handleSearchInput);
  searchInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') {
      performNameSearch();
    }
  });
  searchClearBtn.addEventListener('click', clearSearch);

  // View More functionality
  document.getElementById('view-more-btn').addEventListener('click', loadMoreResults);

  // Close modal when clicking outside
  settingsModal.addEventListener('click', (e) => {
    if (e.target === settingsModal) {
      closeSettings();
    }
  });

  // Close movie details modal when clicking outside
  document.getElementById('movie-details-modal').addEventListener('click', (e) => {
    if (e.target === document.getElementById('movie-details-modal')) {
      closeMovieDetails();
    }
  });

  // Enter key in API key input
  apiKeyInput.addEventListener('keypress', (e) => {
    if (e.key === 'Enter') {
      saveApiKey();
    }
  });

  // Event delegation for result card buttons
  document.addEventListener('click', async (e) => {
    const button = e.target.closest('[data-action]');
    if (!button) return;

    const action = button.dataset.action;
    const id = parseInt(button.dataset.id);
    const contentType = button.dataset.type;
    const title = button.dataset.title;
    const overview = button.dataset.overview;
    const posterPath = button.dataset.posterPath;
    const date = button.dataset.date;
    const voteAverage = parseFloat(button.dataset.voteAverage) || 0;

    console.log('Button clicked:', { action, id, title, contentType });

    // Prevent event bubbling for action buttons
    if (action === 'remove-watched' || action === 'remove-whitelist' || action === 'watch' || action === 'whitelist' || action === 'watch-from-whitelist') {
      e.stopPropagation();
    }

    if (button.disabled) return;
    if (button.tagName === 'BUTTON') button.disabled = true;
    try {
      if (action === 'watch') {
        await toggleWatchStatus(id, title, overview, posterPath, date, voteAverage, contentType);
      } else if (action === 'whitelist') {
        await toggleWhiteListStatus(id, title, overview, posterPath, date, voteAverage, contentType);
      } else if (action === 'remove-watched') {
        await removeFromWatched(id, contentType);
      } else if (action === 'remove-whitelist') {
        await removeFromWhiteList(id, contentType);
      } else if (action === 'view-details') {
        await openMovieDetails(id, contentType, title);
      } else if (action === 'watch-from-whitelist') {
        // Move from waitlist to watched
        await addToWatched(id, title, overview, posterPath, date, voteAverage, contentType);
        await removeFromWhiteList(id, contentType);
        // Refresh the white list display
        if (currentPageType === 'white-list') {
          displayWhiteListItems();
        }
        showSuccess(`Moved "${title}" to watched list!`);
      }
    } catch (error) {
      console.error('Error handling button click:', error);
      console.error('Change was not saved', error);
    } finally { if (button.tagName === 'BUTTON') button.disabled = false; }
  });
}

function switchPage(pageType) {
  currentPageType = pageType;
  window.scrollTo({top:0,behavior:'instant'});
  document.querySelectorAll('.header-nav .nav-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.page === pageType);
    btn.setAttribute('aria-current', btn.dataset.page === pageType ? 'page' : 'false');
  });
  document.querySelectorAll('.main-content > .page-content').forEach(page => page.style.display = page.id === `${pageType}-page` ? 'block' : 'none');
  if (pageType === 'watched') { loadWatchedItems().then(displayWatchedItems); }
  if (pageType === 'white-list') { loadWhiteListItems().then(displayWhiteListItems); }
  if (['research', 'picks', 'white-list'].includes(pageType)) refreshAi();
}

async function loadWatchedItems() {
  try {
    watchedItems = await invoke('get_watched_items');
    updateWatchedStats();
  } catch (error) {
    console.error('Failed to load watched items:', error);
    showError(`Could not load watched history: ${error}`);
  }
}

function updateWatchedStats() {
  const movieCount = watchedItems.filter(item => item.content_type === 'movie').length;
  const tvCount = watchedItems.filter(item => item.content_type === 'tv').length;
  const totalCount = watchedItems.length;

  if (watchedStats) {
    watchedStats.textContent = `${totalCount} items watched (${movieCount} movies, ${tvCount} TV shows)`;
  }
}

function switchWatchedFilter(filter) {
  currentWatchedFilter = filter;

  // Update filter buttons
  document.querySelectorAll('[data-filter]').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.filter === filter);
  });

  displayWatchedItems();
}

function displayWatchedItems() {
  // Filter watched items based on current filter
  if (currentWatchedFilter === 'all') {
    filteredWatchedItems = watchedItems;
  } else {
    filteredWatchedItems = watchedItems.filter(item => item.content_type === currentWatchedFilter);
  }

  if (filteredWatchedItems.length === 0) {
    watchedGrid.style.display = 'none';
    emptyWatched.style.display = 'block';
  } else {
    watchedGrid.style.display = 'grid';
    emptyWatched.style.display = 'none';

    const watchedHtml = filteredWatchedItems.map(item => renderWatchedCard(item)).join('');
    watchedGrid.innerHTML = watchedHtml;
  }
}

function renderWatchedCard(item) { return libraryCard(item, 'watched'); }

// White List Functions (duplicated from watched)
async function loadWhiteListItems() {
  try {
    whiteListItems = await invoke('get_white_list_items');
    updateWhiteListStats();
  } catch (error) {
    console.error('Failed to load white list items:', error);
    showError(`Could not load waitlist: ${error}`);
  }
}

function updateWhiteListStats() {
  const movieCount = whiteListItems.filter(item => item.content_type === 'movie').length;
  const tvCount = whiteListItems.filter(item => item.content_type === 'tv').length;
  const totalCount = whiteListItems.length;

  if (whiteListStats) {
    whiteListStats.textContent = `${totalCount} titles saved (${movieCount} movies, ${tvCount} TV shows)`;
  }
}

function switchWhiteListFilter(filter) {
  currentWhiteListFilter = filter;

  // Update filter buttons
  document.querySelectorAll('.white-list-filters .nav-btn').forEach(btn => {
    btn.classList.remove('active');
  });

  // Find and activate the correct button
  const buttons = document.querySelectorAll('.white-list-filters .nav-btn');
  if (filter === 'all') buttons[0].classList.add('active');
  else if (filter === 'movie') buttons[1].classList.add('active');
  else if (filter === 'tv') buttons[2].classList.add('active');

  displayWhiteListItems();
}

function displayWhiteListItems() {
  // Filter white list items based on current filter
  if (currentWhiteListFilter === 'all') {
    filteredWhiteListItems = whiteListItems;
  } else {
    filteredWhiteListItems = whiteListItems.filter(item => item.content_type === currentWhiteListFilter);
  }

  if (filteredWhiteListItems.length === 0) {
    whiteListGrid.style.display = 'none';
    emptyWhiteList.style.display = 'block';
  } else {
    whiteListGrid.style.display = 'grid';
    emptyWhiteList.style.display = 'none';

    const whiteListHtml = filteredWhiteListItems.map(item => renderWhiteListCard(item)).join('');
    whiteListGrid.innerHTML = whiteListHtml;
  }
}

function renderWhiteListCard(item) { return libraryCard(item, 'waitlist'); }

async function addToWhiteList(id, title, overview, posterPath, releaseDate, voteAverage, contentType) {
  try {
    await invoke('add_white_list_item', {
      id,
      title,
      overview,
      posterPath,
      releaseDate,
      voteAverage,
      contentType
    });

    // Reload white list items
    await loadWhiteListItems();

    // Update the button in the current view
    updateWhiteListButton(id, contentType, true);

    showSuccess(`Added "${title}" to waitlist!`);
  } catch (error) {
    showError(`Failed to add to waitlist: ${error}`);
    throw error;
  }
}

async function removeFromWhiteList(id, contentType) {
  try {
    await invoke('remove_white_list_item', { id, contentType });

    // Reload white list items
    await loadWhiteListItems();

    // Update the button in the current view
    updateWhiteListButton(id, contentType, false);

    // Refresh white list page if currently viewing it
    if (currentPageType === 'white-list') {
      displayWhiteListItems();
    }

    showSuccess('Removed from waitlist!');
  } catch (error) {
    showError(`Failed to remove from waitlist: ${error}`);
    throw error;
  }
}

function updateWhiteListButton(id, contentType, isWhiteListed) {
  const buttons = document.querySelectorAll(`[data-action="whitelist"][data-id="${id}"][data-type="${contentType}"]`);
  buttons.forEach(button => {
    if (isWhiteListed) {
      button.classList.add('white-listed');
      button.textContent = '⭐';
      button.title = 'Remove from waitlist';
    } else {
      button.classList.remove('white-listed');
      button.textContent = '☆';
      button.title = 'Add to waitlist';
    }
  });
}

async function isItemWhiteListed(id, contentType) {
  try {
    return await invoke('is_white_listed', { id, contentType });
  } catch (error) {
    console.error('Failed to check white list status:', error);
    return false;
  }
}

async function addToWatched(id, title, overview, posterPath, releaseDate, voteAverage, contentType) {
  try {
    await invoke('add_watched_item', {
      id,
      title,
      overview,
      posterPath,
      releaseDate,
      voteAverage,
      contentType
    });

    // Reload watched items
    await loadWatchedItems();

    // Update the button in the current view
    updateWatchButton(id, contentType, true);

    showSuccess(`Added "${title}" to watched list!`);
  } catch (error) {
    showError(`Failed to add to watched: ${error}`);
    throw error;
  }
}

async function removeFromWatched(id, contentType) {
  try {
    await invoke('remove_watched_item', { id, contentType });

    // Reload watched items
    await loadWatchedItems();

    // Update the button in the current view
    updateWatchButton(id, contentType, false);

    // Refresh watched page if currently viewing it
    if (currentPageType === 'watched') {
      displayWatchedItems();
    }

    showSuccess('Removed from watched list!');
  } catch (error) {
    showError(`Failed to remove from watched: ${error}`);
    throw error;
  }
}

function updateWatchButton(id, contentType, isWatched) {
  const buttons = document.querySelectorAll(`[data-action="watch"][data-id="${id}"][data-type="${contentType}"]`);
  buttons.forEach(button => {
    if (isWatched) {
      button.classList.add('watched');
      button.textContent = '✅';
      button.title = 'Remove from watched';
    } else {
      button.classList.remove('watched');
      button.textContent = '👁️';
      button.title = 'Mark as watched';
    }
  });
}

async function isItemWatched(id, contentType) {
  try {
    return await invoke('is_watched', { id, contentType });
  } catch (error) {
    console.error('Failed to check watched status:', error);
    return false;
  }
}

async function loadApiKey() {
  try {
    const apiKey = await invoke('load_api_key');
    if (apiKey) {
    apiKeyInput.value = apiKey;
    }
  } catch (error) {
    console.error('Failed to load API key:', error);
  }
}

async function saveApiKey() {
  const apiKey = apiKeyInput.value.trim();

  if (!apiKey) {
    showError('Please enter a valid API key');
    return;
  }

  try {
    await invoke('save_api_key', { apiKey });
    closeSettings();
    await Promise.all([loadGenres(), loadLanguages(), loadWatchProviders()]);
    await performSearch();
    checkForNewSeasons();
    showSuccess('API key saved successfully!');
  } catch (error) {
    showError(`Failed to save API key: ${error}`);
  }
}

function openSettings() {
  settingsModal.classList.add('active');
  apiKeyInput.focus();
  refreshMcp();
}

function closeSettings() {
  settingsModal.classList.remove('active');
}

async function switchContentType(type) {
  ++discoverGeneration;
  const generation = ++contentGeneration;
  currentContentType = type;
  selectedGenres = [];
  selectedProviders = [];
  // Note: selectedLanguage is kept across content types

  // Update toggle buttons
  document.querySelectorAll('[data-type].toggle-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.type === type);
    btn.setAttribute('aria-pressed', String(btn.dataset.type === type));
  });

  // Update sort options
  const sortSelect = document.getElementById('sort-by');
  const movieOptions = [
    { value: 'release_date.desc', text: 'Release Date (Newest First)' },
    { value: 'popularity.desc', text: 'Popularity (High to Low)' },
    { value: 'popularity.asc', text: 'Popularity (Low to High)' },
    { value: 'vote_average.desc', text: 'Rating (High to Low)' },
    { value: 'vote_average.asc', text: 'Rating (Low to High)' },
    { value: 'release_date.asc', text: 'Release Date (Oldest First)' }
  ];

  const tvOptions = [
    { value: 'first_air_date.desc', text: 'Air Date (Newest First)' },
    { value: 'popularity.desc', text: 'Popularity (High to Low)' },
    { value: 'popularity.asc', text: 'Popularity (Low to High)' },
    { value: 'vote_average.desc', text: 'Rating (High to Low)' },
    { value: 'vote_average.asc', text: 'Rating (Low to High)' },
    { value: 'first_air_date.asc', text: 'Air Date (Oldest First)' }
  ];

  const options = type === 'movie' ? movieOptions : tvOptions;
  const defaultSort = type === 'movie' ? 'release_date.desc' : 'first_air_date.desc';

  sortSelect.innerHTML = options.map(opt =>
    `<option value="${opt.value}">${opt.text}</option>`
  ).join('');

  sortSelect.value = defaultSort;

  updateFilterSummary();
  await Promise.all([loadGenres(), loadWatchProviders()]);
  if (generation !== contentGeneration) return;
  clearResults();
  performSearch();
}

async function loadGenres() {
  const type = currentContentType;
  try {
    genresContainer.textContent = 'Loading genres…';
    const genres = await invoke(type === 'movie' ? 'get_movie_genres' : 'get_tv_genres');
    if (type !== currentContentType) return;
    allGenres[type] = genres;
    renderGenres(genres);
    updateFilterSummary();
  } catch (error) { if (type === currentContentType) genresContainer.textContent = `Could not load genres: ${error}`; }
}

function renderGenres(genres) {
  genresContainer.innerHTML = genres.map(genre => `<button type="button" class="genre-chip ${selectedGenres.includes(genre.id) ? 'selected' : ''}" aria-pressed="${selectedGenres.includes(genre.id)}" data-id="${genre.id}">${escapeHtml(genre.name)}</button>`).join('');
  genresContainer.querySelectorAll('.genre-chip').forEach(chip => chip.addEventListener('click', () => toggleGenre(Number(chip.dataset.id))));
}

function toggleGenre(genreId) {
  const index = selectedGenres.indexOf(genreId);
  if (index > -1) {
    selectedGenres.splice(index, 1);
  } else {
    selectedGenres.push(genreId);
  }

  // Update UI
  genresContainer.querySelectorAll('.genre-chip').forEach(chip => {
    const id = parseInt(chip.dataset.id);
    chip.classList.toggle('selected', selectedGenres.includes(id));
    chip.setAttribute('aria-pressed', String(selectedGenres.includes(id)));
    updateFilterSummary();
  });
}

async function loadLanguages() {
  try {
    languageContainer.textContent = 'Loading languages…';
    const languages = (await invoke('get_languages')).sort((a,b) => a.english_name.localeCompare(b.english_name));
    languageContainer.innerHTML = `<button class="language-chip ${!selectedLanguage ? 'selected' : ''}" data-id="">Any language</button>` + languages.map(lang => `<button class="language-chip ${selectedLanguage === lang.iso_639_1 ? 'selected' : ''}" data-id="${escapeHtml(lang.iso_639_1)}">${escapeHtml(lang.english_name)}</button>`).join('');
    languageContainer.querySelectorAll('.language-chip').forEach(chip => chip.addEventListener('click', () => toggleLanguage(chip.dataset.id || null)));
    updateFilterSummary();
  } catch (error) { languageContainer.textContent = `Could not load languages: ${error}`; }
}

function toggleLanguage(langCode) {
  if (selectedLanguage === langCode) {
    selectedLanguage = null; // Deselect
  } else {
    selectedLanguage = langCode; // Select
  }

  languageContainer.querySelectorAll('.language-chip').forEach(chip => {
    chip.classList.toggle('selected', (chip.dataset.id || null) === selectedLanguage);
    chip.setAttribute('aria-pressed', String((chip.dataset.id || null) === selectedLanguage));
  });
  document.getElementById('language-dropdown').open = false;
  updateFilterSummary();
}

async function loadWatchProviders() {
  const type = currentContentType;
  try {
    providersContainer.textContent = 'Loading providers…';
    const providers = await invoke('get_watch_providers', { contentType: type });
    if (type !== currentContentType) return;
    providersContainer.innerHTML = providers.sort((a,b) => a.provider_name.localeCompare(b.provider_name)).map(provider => `<button class="provider-chip ${selectedProviders.includes(provider.provider_id) ? 'selected' : ''}" data-id="${provider.provider_id}" aria-pressed="${selectedProviders.includes(provider.provider_id)}">${provider.logo_path ? `<img src="https://image.tmdb.org/t/p/w92${escapeHtml(provider.logo_path)}" alt="" loading="lazy">` : ''}${escapeHtml(provider.provider_name)}</button>`).join('');
    providersContainer.querySelectorAll('.provider-chip').forEach(chip => chip.addEventListener('click', () => toggleProvider(Number(chip.dataset.id))));
    updateFilterSummary();
  } catch (error) { if (type === currentContentType) providersContainer.textContent = `Could not load providers: ${error}`; }
}

function toggleProvider(providerId) {
  const index = selectedProviders.indexOf(providerId);
  if (index > -1) {
    selectedProviders.splice(index, 1);
  } else {
    selectedProviders.push(providerId);
  }

  providersContainer.querySelectorAll('.provider-chip').forEach(chip => {
    const id = parseInt(chip.dataset.id);
    chip.classList.toggle('selected', selectedProviders.includes(id));
    chip.setAttribute('aria-pressed', String(selectedProviders.includes(id)));
    updateFilterSummary();
  });
}

async function performSearch() {
  const yearFromInput = document.getElementById('year-from').value;
  const yearToInput = document.getElementById('year-to').value;
  const sortBy = document.getElementById('sort-by').value;
  const excludeAnimation = document.getElementById('exclude-animation').checked;
  const minRating = parseFloat(document.getElementById('min-rating').value) || 0;

  // Parse year values - left side is START year (from), right side is END year (to)
  let yearFrom = yearFromInput ? Number(yearFromInput) : null;
  let yearTo = yearToInput ? Number(yearToInput) : null;

  // Ensure yearFrom is always less than or equal to yearTo
  if (yearFrom !== null && yearTo !== null && yearFrom > yearTo) {
    [yearFrom, yearTo] = [yearTo, yearFrom];
  }

  currentFilters = {
    yearFrom: yearFrom,
    yearTo: yearTo,
    sortBy: sortBy,
    genreIds: [...selectedGenres],
    excludeAnimation: excludeAnimation,
    watchProviders: [...selectedProviders],
    originalLanguage: selectedLanguage,
    minRating: minRating
  };

  document.querySelectorAll('.filter-dropdown').forEach(d => d.open = false);
  ++discoverGeneration;
  updateFilterSummary();
  // Reset for new search
  currentPage = 1;
  allResults = [];
  await searchContent();
}

async function loadMoreResults() {
  if (currentPage < totalPages && !document.getElementById('view-more-btn').disabled) await searchContent(true);
}

async function searchContent(appendResults = false) {
  const generation = discoverGeneration;
  const type = currentContentType;
  const page = appendResults ? currentPage + 1 : 1;
  const filters = { ...currentFilters };
  const more = document.getElementById('view-more-btn');
  document.getElementById('search-btn').disabled = true;
  try {
    if (!appendResults) { resultsGrid.innerHTML = ''; showLoading(); }
    else { more.textContent = 'Loading…'; more.disabled = true; }
    const response = await invoke(type === 'movie' ? 'search_movies' : 'search_tv_shows', {
      query: '', page, yearFrom: filters.yearFrom, yearTo: filters.yearTo, genreIds: filters.genreIds,
      sortBy: filters.sortBy, excludeAnimation: filters.excludeAnimation, withWatchProviders: filters.watchProviders,
      withOriginalLanguage: filters.originalLanguage, minRating: filters.minRating
    });
    if (generation !== discoverGeneration || type !== currentContentType) return;
    currentPage = page;
    totalPages = Math.min(500, response.total_pages);
    const fresh = response.results.filter(item => !appendResults || !allResults.some(existing => existing.id === item.id));
    allResults = appendResults ? [...allResults, ...fresh] : fresh;
    await displayResults({ ...response, results: allResults }, appendResults, fresh);
  } catch (error) {
    if (generation !== discoverGeneration) return;
    if (!appendResults) { hideAllStates(); errorMessage.textContent = `Search failed: ${error}`; errorState.style.display = 'block'; }
    else showError(`Could not load more: ${error}`);
  } finally {
    if (generation === discoverGeneration) {
      document.getElementById('search-btn').disabled = false;
      more.disabled = false; more.textContent = 'View more titles';
    }
  }
}

function showLoading() {
  hideAllStates();
  loading.style.display = 'block';
}

function hideAllStates() {
  loading.style.display = 'none';
  emptyState.style.display = 'none';
  errorState.style.display = 'none';
  resultsHeader.style.display = 'none';
  viewMoreContainer.style.display = 'none';
}

function showError(message) { notify(message, 'error'); }

function showSuccess(message) { notify(message, 'success'); }

function displayResults(response, isAppending = false, newResults = []) {
  if (!isAppending) {
  hideAllStates();
  }

  if (!response.results || response.results.length === 0) {
    if (!isAppending) {
      emptyState.querySelector('h3').textContent = 'No Results Found';
      emptyState.querySelector('p').textContent = 'Try adjusting your filters or search criteria.';
    emptyState.style.display = 'block';
    }
    return;
  }

  // Show results header
  resultsHeader.style.display = 'block';
  resultsInfo.textContent = `Showing ${allResults.length.toLocaleString()} of ${response.total_results.toLocaleString()} results`;

  // Render results
  if (isAppending) {
    // Append only the new results from the current page
    const newResultsHtml = newResults.map(item => renderResultCard(item));
    resultsGrid.innerHTML += newResultsHtml.join('');
  } else {
    // Replace all results
    const resultsHtml = response.results.map(item => renderResultCard(item));
  resultsGrid.innerHTML = resultsHtml.join('');
  }

  // Show/hide view more button
  if (currentPage < totalPages) {
    viewMoreContainer.style.display = 'flex';
    const viewMoreBtn = document.getElementById('view-more-btn');
    viewMoreBtn.textContent = '📺 View More Results';
    viewMoreBtn.disabled = false;
  } else {
    viewMoreContainer.style.display = 'none';
  }

  // Keep the compact controls and movie row visible together; do not scroll on startup.
}

function renderResultCard(item) { return libraryCard({ ...item, content_type: currentContentType }, 'browse'); }

function clearResults() {
  resultsGrid.innerHTML = '';
  allResults = [];
  currentPage = 1;
  hideAllStates();
  emptyState.querySelector('h3').textContent = 'Welcome to MoviNight!';
  emptyState.querySelector('p').textContent = 'Use the filters above to discover amazing movies and TV shows.';
  emptyState.style.display = 'block';
}

// Global functions (accessible from HTML onclick handlers)
async function toggleWatchStatus(id, title, overview, posterPath, releaseDate, voteAverage, contentType) {
  const isWatched = watchedItems.some(item => item.id === id && item.content_type === contentType);

  if (isWatched) {
    await removeFromWatched(id, contentType);
  } else {
    await addToWatched(id, title, overview, posterPath, releaseDate, voteAverage, contentType);
  }
}

async function toggleWhiteListStatus(id, title, overview, posterPath, releaseDate, voteAverage, contentType) {
  const isWhiteListed = whiteListItems.some(item => item.id === id && item.content_type === contentType);

  if (isWhiteListed) {
    await removeFromWhiteList(id, contentType);
  } else {
    await addToWhiteList(id, title, overview, posterPath, releaseDate, voteAverage, contentType);
  }
}

// Search input functions
function handleSearchInput() {
  const hasValue = searchInput.value.trim().length > 0;
  searchClearBtn.classList.toggle('visible', hasValue);
  if (nameLoading) { ++nameGeneration; nameLoading = false; document.getElementById('search-name-btn').disabled = false; searchNameLoading.style.display = 'none'; }
}

function clearSearch() {
  searchInput.value = '';
  searchClearBtn.classList.remove('visible');
  searchInput.focus();
  clearSearchResults();
}

function clearSearchResults() {
  ++nameGeneration;
  nameLoading = false;
  document.getElementById('search-name-btn').disabled = false;
  searchNameEmpty.querySelector('h3').textContent = 'Search for Movies & TV Shows';
  searchNameEmpty.querySelector('p').textContent = 'Enter a title above to get started.';
  searchResults = [];
  searchPage = 1;
  searchTotalPages = 1;
  hideSearchStates();
  searchNameEmpty.style.display = 'block';
}

function hideSearchStates() {
  searchNameLoading.style.display = 'none';
  searchNameEmpty.style.display = 'none';
  searchNameError.style.display = 'none';
  searchNameResults.innerHTML = '';
}

async function performNameSearch() {
  const query = searchInput.value.trim();
  if (!query) { clearSearchResults(); return; }
  ++nameGeneration;
  nameQuery = query;
  nameLoading = false;
  searchPage = 0; searchResults = []; nameTotals = { movie: 500, tv: 500 };
  await loadNamePage(false);
}

function displaySearchResults(results, totalResults) {
  searchNameResults.innerHTML = `<div class="search-results-header"><h3>Search results</h3><div class="search-results-info">${results.length} shown · ${totalResults.toLocaleString()} matches</div></div><div class="search-results-grid">${results.map(renderSearchResultCard).join('')}</div>${searchPage < searchTotalPages ? '<div class="view-more-container"><button class="btn btn-secondary" id="name-more">View more titles</button></div>' : ''}`;
  document.getElementById('name-more')?.addEventListener('click', () => loadNamePage(true));
}

function renderSearchResultCard(item) { return libraryCard(item, 'browse'); }

// Keep filter functions available globally for HTML onclick handlers (still used in filter buttons)
window.switchWhiteListFilter = switchWhiteListFilter;

// Year Range Initialization and Management
function initializeYearRange() {
  const currentYear = new Date().getFullYear();
  const yearFromInput = document.getElementById('year-from');
  const yearToInput = document.getElementById('year-to');
  const yearFromArrow = document.getElementById('year-from-arrow');
  const yearToArrow = document.getElementById('year-to-arrow');
  const yearFromDropdown = document.getElementById('year-from-dropdown');
  const yearToDropdown = document.getElementById('year-to-dropdown');
  const yearFromContent = yearFromDropdown.querySelector('.year-dropdown-content');
  const yearToContent = yearToDropdown.querySelector('.year-dropdown-content');

  // Set default values: From = 2015, To = current year
  yearFromInput.value = '';
  yearToInput.value = '';

  // Populate dropdowns with year options (from current year down to 1900)
  const yearItemsHtml = generateYearDropdownItems(currentYear);
  yearFromContent.innerHTML = yearItemsHtml;
  yearToContent.innerHTML = yearItemsHtml;

  // Toggle dropdown for "From" year
  yearFromArrow.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleDropdown(yearFromDropdown, yearToDropdown);
  });

  // Toggle dropdown for "To" year
  yearToArrow.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleDropdown(yearToDropdown, yearFromDropdown);
  });

  // Handle year selection from dropdowns
  yearFromContent.addEventListener('click', (e) => {
    if (e.target.classList.contains('year-dropdown-item')) {
      yearFromInput.value = e.target.dataset.year;
      yearFromDropdown.classList.remove('open');
    }
  });

  yearToContent.addEventListener('click', (e) => {
    if (e.target.classList.contains('year-dropdown-item')) {
      yearToInput.value = e.target.dataset.year;
      yearToDropdown.classList.remove('open');
    }
  });

  // Close dropdowns when clicking outside
  document.addEventListener('click', () => {
    yearFromDropdown.classList.remove('open');
    yearToDropdown.classList.remove('open');
  });

  // Prevent closing when clicking inside dropdown
  yearFromDropdown.addEventListener('click', (e) => e.stopPropagation());
  yearToDropdown.addEventListener('click', (e) => e.stopPropagation());

  // Validate input on change
  yearFromInput.addEventListener('change', () => validateYearInput(yearFromInput));
  yearToInput.addEventListener('change', () => validateYearInput(yearToInput));
}

function toggleDropdown(dropdownToToggle, dropdownToClose) {
  dropdownToClose.classList.remove('open');
  dropdownToToggle.classList.toggle('open');
}

function generateYearDropdownItems(currentYear) {
  let items = '';
  // Generate years from current year down to 1900
  for (let year = currentYear; year >= 1900; year--) {
    items += `<div class="year-dropdown-item" data-year="${year}">${year}</div>`;
  }
  return items;
}

function validateYearInput(input) {
  let value = parseInt(input.value);
  if (!input.value) return;
  if (isNaN(value) || value < 1900) {
    value = 1900;
  } else if (value > 2100) {
    value = 2100;
  }
  input.value = value;
}

// Movie Details Modal Functions
async function openMovieDetails(id, contentType, title) {
  const modal = document.getElementById('movie-details-modal');
  const modalTitle = document.getElementById('movie-details-title');
  const loading = document.getElementById('movie-details-loading');
  const content = document.getElementById('movie-details-content');
  const error = document.getElementById('movie-details-error');

  // Season tracker elements
  const seasonPanel = document.getElementById('season-tracker-panel');
  const seasonLoading = document.getElementById('season-tracker-loading');
  const seasonContent = document.getElementById('season-tracker-content');
  const seasonError = document.getElementById('season-tracker-error');

  const showSeasons = contentType === 'tv' && currentPageType === 'watched';

  // Set modal title and show modal
  modalTitle.textContent = `${contentType === 'movie' ? 'Movie' : 'TV Show'} Details`;
  if (showSeasons) {
    modal.querySelector('.movie-details-modal').classList.add('with-seasons');
    seasonPanel.style.display = 'flex';
    seasonLoading.style.display = 'block';
    seasonContent.style.display = 'none';
    seasonError.style.display = 'none';
    currentSeasonTrackerId = id;
  } else {
    modal.querySelector('.movie-details-modal').classList.remove('with-seasons');
    seasonPanel.style.display = 'none';
    currentSeasonTrackerId = null;
  }

  modal.classList.add('active');

  // Reset states
  loading.style.display = 'block';
  content.style.display = 'none';
  error.style.display = 'none';

  try {
    // Determine which API calls to make
    const promises = [
      contentType === 'movie' ? invoke('get_movie_details', { id }) : invoke('get_tv_details', { id }),
      invoke('get_trailers', { id, contentType })
    ];

    if (showSeasons) {
      promises.push(invoke('get_tv_season_details', { id }));
    }

    // Fetch details in parallel
    const results = await Promise.all(promises);
    const details = results[0];
    const trailers = results[1];

    console.log('Movie details:', details);
    console.log('Trailers:', trailers);

    // Populate the modal
    populateMovieDetails(details, trailers, contentType);

    // Show content
    loading.style.display = 'none';
    content.style.display = 'block';

    // Handle season tracker data if applicable
    if (showSeasons && results[2]) {
      const seasonData = results[2];
      console.log('Season details:', seasonData);
      renderSeasonTracker(seasonData);
      seasonLoading.style.display = 'none';
      seasonContent.style.display = 'flex';
    }

  } catch (err) {
    console.error('Failed to load movie details:', err);
    loading.style.display = 'none';
    error.style.display = 'block';
    document.getElementById('movie-details-error-message').textContent = err.toString();

    if (showSeasons) {
      seasonLoading.style.display = 'none';
      seasonError.style.display = 'block';
      document.getElementById('season-tracker-error-message').textContent = 'Failed to load details.';
    }
  }
}

function populateMovieDetails(details, trailers, contentType) {
  // Basic info
  const mainTitle = document.getElementById('movie-details-main-title');
  const tagline = document.getElementById('movie-details-tagline');
  const poster = document.getElementById('movie-details-poster');
  const year = document.getElementById('movie-details-year');
  const rating = document.getElementById('movie-details-rating');
  const type = document.getElementById('movie-details-type');
  const runtime = document.getElementById('movie-details-runtime');
  const genres = document.getElementById('movie-details-genres');
  const overview = document.getElementById('movie-details-overview');

  // Set title
  const title = contentType === 'movie' ? details.title : details.name;
  mainTitle.textContent = title;

  // Set tagline
  if (details.tagline) {
    tagline.textContent = details.tagline;
    tagline.style.display = 'block';
  } else {
    tagline.style.display = 'none';
  }

  // Set poster
  const posterUrl = details.poster_path
    ? `https://image.tmdb.org/t/p/w500${details.poster_path}`
    : 'data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgZmlsbD0iI2UyZThmMCIvPjx0ZXh0IHg9IjUwJSIgeT0iNTAlIiBmb250LWZhbWlseT0iQXJpYWwsIHNhbnMtc2VyaWYiIGZvbnQtc2l6ZT0iMjQiIGZpbGw9IiM3MTgwOTYiIHRleHQtYW5jaG9yPSJtaWRkbGUiIGR5PSIwLjNlbSI+SW1hZ2UgTm90IEF2YWlsYWJsZTwvdGV4dD48L3N2Zz4=';
  poster.src = posterUrl;
  poster.alt = `${title} Poster`;

  // Set metadata
  const releaseDate = contentType === 'movie' ? details.release_date : details.first_air_date;
  const releaseYear = releaseDate ? new Date(releaseDate).getFullYear() : 'N/A';
  year.textContent = `${releaseYear}`;

  const voteAverage = details.vote_average !== null ? details.vote_average.toFixed(1) : 'N/A';
  rating.textContent = `${voteAverage}`;

  type.textContent = contentType === 'movie' ? '🎬 Movie' : '📺 TV Show';

  if (contentType === 'movie' && details.runtime) {
    const hours = Math.floor(details.runtime / 60);
    const minutes = details.runtime % 60;
    runtime.textContent = `⏱️ ${hours}h ${minutes}m`;
    runtime.style.display = 'inline-block';
  } else if (contentType === 'tv') {
    runtime.textContent = `📺 ${details.number_of_seasons} Season${details.number_of_seasons !== 1 ? 's' : ''} • ${details.number_of_episodes} Episodes`;
    runtime.style.display = 'inline-block';
  } else {
    runtime.style.display = 'none';
  }

  // Set genres
  if (details.genres && details.genres.length > 0) {
    genres.innerHTML = details.genres
      .map(genre => `<span class="movie-genre-chip">${escapeHtml(genre.name)}</span>`)
      .join('');
  } else {
    genres.innerHTML = '';
  }

  // Set overview
  overview.textContent = details.overview || 'No overview available.';

  // Handle trailers
  setupTrailers(trailers);
}

function setupTrailers(trailers) {
  const trailerSection = document.getElementById('movie-trailer-section');
  const noTrailerSection = document.getElementById('movie-no-trailer');
  const trailerIframe = document.getElementById('trailer-iframe');

  if (trailers && trailers.length > 0) {
    // Use the first trailer (should be the best one due to backend sorting)
    const trailer = trailers[0];
    const embedUrl = `https://www.youtube.com/embed/${trailer.key}?rel=0&showinfo=0&modestbranding=1`;

    trailerIframe.src = embedUrl;
    trailerSection.style.display = 'block';
    noTrailerSection.style.display = 'none';
  } else {
    trailerSection.style.display = 'none';
    noTrailerSection.style.display = 'block';
  }
}

function closeMovieDetails() {
  const modal = document.getElementById('movie-details-modal');
  const trailerIframe = document.getElementById('trailer-iframe');

  // Stop the trailer
  trailerIframe.src = '';

  // Reset split layout classes
  modal.querySelector('.movie-details-modal').classList.remove('with-seasons');
  document.getElementById('season-tracker-panel').style.display = 'none';

  // Hide modal
  modal.classList.remove('active');
  currentSeasonTrackerId = null;
}

// ============================================================
// SEASON TRACKER SYSTEM
// ============================================================

function renderSeasonTracker(data) {
  const seasonList = document.getElementById('season-list');
  const summary = document.getElementById('season-tracker-summary');
  const watchedSeasons = data.watched_seasons || [];
  const totalSeasons = data.seasons.length;
  const watchedCount = watchedSeasons.length;
  const progressPercent = totalSeasons > 0 ? Math.round((watchedCount / totalSeasons) * 100) : 0;

  // Summary with progress bar
  summary.innerHTML = `
    <span>${totalSeasons} season${totalSeasons !== 1 ? 's' : ''} total</span>
    <div class="summary-progress">
      <div class="progress-bar-container">
        <div class="progress-bar-fill" style="width: ${progressPercent}%"></div>
      </div>
      <span class="progress-text">${watchedCount}/${totalSeasons} watched</span>
    </div>
  `;

  // Select all / deselect all buttons + season rows
  let html = `
    <div class="season-select-all">
      <button class="btn btn-secondary" onclick="selectAllSeasons(true)">✅ Select All</button>
      <button class="btn btn-secondary" onclick="selectAllSeasons(false)">⬜ Deselect All</button>
    </div>
  `;

  data.seasons.forEach(season => {
    const isChecked = watchedSeasons.includes(season.season_number);
    // A season is "new" if it wasn't known when user last saved
    // (season_number > total_seasons_known, or total_seasons_known was 0/null meaning never tracked)
    const isNew = data.total_seasons_known > 0 && season.season_number > data.total_seasons_known && !isChecked;
    const airDate = season.air_date ? new Date(season.air_date).toLocaleDateString('en-US', { year: 'numeric', month: 'short' }) : 'TBA';

    html += `
      <label class="season-row ${isNew ? 'is-new' : ''}" for="season-cb-${season.season_number}">
        <div class="season-checkbox-wrapper">
          <input type="checkbox" class="season-checkbox" id="season-cb-${season.season_number}"
                 data-season="${season.season_number}" ${isChecked ? 'checked' : ''}
                 onchange="updateSeasonSummary()">
          <div class="season-checkbox-visual"></div>
        </div>
        <div class="season-info">
          <div class="season-name">${escapeHtml(season.name)}</div>
          <div class="season-meta">
            <span>📅 ${airDate}</span>
            <span>🎬 ${season.episode_count} episode${season.episode_count !== 1 ? 's' : ''}</span>
          </div>
        </div>
        ${isNew ? '<span class="new-tag">NEW</span>' : ''}
      </label>
    `;
  });

  seasonList.innerHTML = html;
}

function updateSeasonSummary() {
  const checkboxes = document.querySelectorAll('.season-checkbox');
  const total = checkboxes.length;
  const checked = document.querySelectorAll('.season-checkbox:checked').length;
  const progressPercent = total > 0 ? Math.round((checked / total) * 100) : 0;

  const summary = document.getElementById('season-tracker-summary');
  summary.innerHTML = `
    <span>${total} season${total !== 1 ? 's' : ''} total</span>
    <div class="summary-progress">
      <div class="progress-bar-container">
        <div class="progress-bar-fill" style="width: ${progressPercent}%"></div>
      </div>
      <span class="progress-text">${checked}/${total} watched</span>
    </div>
  `;
}

function selectAllSeasons(selectAll) {
  const checkboxes = document.querySelectorAll('.season-checkbox');
  checkboxes.forEach(cb => { cb.checked = selectAll; });
  updateSeasonSummary();
}

async function saveSeasonTracker() {
  if (!currentSeasonTrackerId) return;

  const checkboxes = document.querySelectorAll('.season-checkbox');
  const watchedSeasons = [];
  let totalSeasons = 0;

  checkboxes.forEach(cb => {
    totalSeasons++;
    if (cb.checked) {
      watchedSeasons.push(parseInt(cb.dataset.season));
    }
  });

  try {
    await invoke('update_watched_seasons', {
      id: currentSeasonTrackerId,
      watchedSeasons: watchedSeasons,
      totalSeasonsKnown: totalSeasons
    });

    // Reload watched items to get updated flags
    await loadWatchedItems();

    // Refresh the watched page display if visible
    if (currentPageType === 'watched') {
      displayWatchedItems();
    }

    closeMovieDetails();
    showSuccess('Season tracking updated!');
  } catch (err) {
    console.error('Failed to save season tracking:', err);
    showError(`Failed to save: ${err}`);
  }
}

// Background check for new seasons on startup
async function checkForNewSeasons() {
  try {
    // Only run if there are watched TV shows
    const hasTvShows = watchedItems.some(item => item.content_type === 'tv');
    if (!hasTvShows) return;

    console.log('🔔 Checking for new seasons in background...');
    const alerts = await invoke('check_all_new_seasons');

    if (alerts.length > 0) {
      console.log(`🔔 Found new seasons for ${alerts.length} show(s):`, alerts);

      // Reload watched items to get updated has_new_seasons flags
      await loadWatchedItems();

      // Refresh watched page if currently viewing it
      if (currentPageType === 'watched') {
        displayWatchedItems();
      }

      // Show a toast notification
      notify(`${alerts.length} ${alerts.length === 1 ? 'show has' : 'shows have'} unwatched seasons. See your Watched list.`, 'info', () => switchPage('watched'));
    } else {
      console.log('🔔 No new seasons found.');
    }
  } catch (err) {
    // Silently fail — this is a background check
    console.error('Background new-season check failed:', err);
  }
}

// Make functions available globally
window.selectAllSeasons = selectAllSeasons;
window.updateSeasonSummary = updateSeasonSummary;
function escapeHtml(value) {
  return String(value ?? '').replace(/[&<>"']/g, char => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
}
function libraryCard(item, mode = 'browse') {
  const title = item.title || item.name || 'Untitled';
  const kind = item.content_type;
  const date = item.release_date || item.first_air_date || '';
  const year = date.slice(0,4) || 'Year unknown';
  const rating = Number(item.vote_average || 0).toFixed(1);
  const watched = watchedItems.some(i => i.id === item.id && i.content_type === kind);
  const waitlisted = whiteListItems.some(i => i.id === item.id && i.content_type === kind);
  const attrs = `data-id="${Number(item.id)}" data-type="${escapeHtml(kind)}" data-title="${escapeHtml(title)}" data-overview="${escapeHtml(item.overview || '')}" data-poster-path="${escapeHtml(item.poster_path || '')}" data-date="${escapeHtml(date)}" data-vote-average="${rating}"`;
  const poster = item.poster_path ? `<img class="result-poster" src="https://image.tmdb.org/t/p/w500${escapeHtml(item.poster_path)}" alt="${escapeHtml(title)}" loading="lazy">` : '<div class="result-poster no-poster">No poster available</div>';
  let buttons = mode === 'watched' ? `<button class="remove-btn" data-action="remove-watched" ${attrs} aria-label="Remove ${escapeHtml(title)} from watched">×</button>`
    : mode === 'waitlist' ? `<button class="watch-btn" data-action="watch-from-whitelist" ${attrs} aria-label="Mark ${escapeHtml(title)} watched">✓</button><button class="remove-btn" data-action="remove-whitelist" ${attrs} aria-label="Remove ${escapeHtml(title)} from waitlist">×</button>`
    : `<button class="watch-btn ${watched ? 'watched' : ''}" data-action="watch" ${attrs} aria-label="${watched ? 'Remove from watched' : 'Mark watched'}">${watched ? '✓' : '◉'}</button><button class="white-list-btn ${waitlisted ? 'white-listed' : ''}" data-action="whitelist" ${attrs} aria-label="${waitlisted ? 'Remove from waitlist' : 'Add to waitlist'}">${waitlisted ? '★' : '☆'}</button>`;
  const history = mode === 'watched' ? `<div class="watched-date">Watched ${escapeHtml(item.watched_date)}</div>` : mode === 'waitlist' ? `<div class="white-list-date">Saved ${escapeHtml(item.white_list_date)}</div>` : '';
  const badge = item.has_new_seasons ? '<span class="new-season-badge">Unwatched seasons</span>' : '';
  return `<article class="result-card ${item.has_new_seasons ? 'has-new-seasons' : ''}" tabindex="0" role="button" aria-label="Details for ${escapeHtml(title)}" data-action="view-details" ${attrs}>${buttons}${badge}${poster}<div class="result-info"><div class="result-title">${escapeHtml(title)}</div><div class="result-meta"><span class="result-year">${escapeHtml(year)} · ${kind === 'tv' ? 'TV' : 'Movie'}</span><span class="result-rating">${rating}</span></div><div class="result-overview">${escapeHtml(item.overview || 'No overview available.')}</div>${history}</div></article>`;
}
function notify(message, type = 'info', action) {
  const stack = document.getElementById('toast-stack');
  if (!stack) return;
  while (stack.children.length >= 3) stack.firstElementChild.remove();
  const toast = document.createElement('div');
  toast.className = `toast toast-${type}`;
  const icon = document.createElement('span'); icon.className = 'toast-icon'; icon.textContent = type === 'error' ? '!' : type === 'success' ? '✓' : 'i';
  const text = document.createElement('p'); text.textContent = message;
  const close = document.createElement('button'); close.textContent = '×'; close.setAttribute('aria-label', 'Dismiss notification'); close.onclick = () => toast.remove();
  toast.append(icon, text);
  if (action) { const view = document.createElement('button'); view.className = 'toast-action'; view.textContent = 'View'; view.onclick = () => { action(); toast.remove(); }; toast.append(view); }
  toast.append(close); stack.append(toast);
  let timer = setTimeout(() => toast.remove(), type === 'error' ? 12000 : 6500);
  toast.onmouseenter = () => clearTimeout(timer);
  toast.onmouseleave = () => { timer = setTimeout(() => toast.remove(), 6500); };
  toast.onfocusin = () => clearTimeout(timer);
  toast.onfocusout = () => { timer = setTimeout(() => toast.remove(), 6500); };
}
function updateFilterSummary() {
  const names = (allGenres[currentContentType] || []).filter(g => selectedGenres.includes(g.id)).map(g => g.name);
  document.getElementById('genres-summary').textContent = names.length ? names.join(' + ') : 'All genres';
  const lang = languageContainer.querySelector(`[data-id="${selectedLanguage || ''}"]`);
  document.getElementById('language-summary').textContent = selectedLanguage ? lang?.textContent || selectedLanguage : 'Any language';
  document.getElementById('providers-summary').textContent = selectedProviders.length ? `${selectedProviders.length} selected` : 'Any provider';
  document.getElementById('filter-status').textContent = `${currentContentType === 'movie' ? 'Movies' : 'TV shows'} · ${names.length ? names.join(' or ') : 'all genres'}`;
}
async function loadNamePage(append) {
  if (nameLoading) return;
  const generation = nameGeneration, query = nameQuery, page = searchPage + 1;
  nameLoading = true;
  const searchButton = document.getElementById('search-name-btn');
  searchButton.disabled = true;
  const more = document.getElementById('name-more');
  if (more) { more.disabled = true; more.textContent = 'Loading…'; }
  if (!append) { hideSearchStates(); searchNameLoading.style.display = 'block'; }
  try {
    const types = ['movie','tv'].filter(type => page <= nameTotals[type]);
    const responses = await Promise.all(types.map(type => invoke(type === 'movie' ? 'search_movies' : 'search_tv_shows', {
      query, page, yearFrom: null, yearTo: null, genreIds: [], sortBy: 'popularity.desc', excludeAnimation: false,
      withWatchProviders: [], withOriginalLanguage: null, minRating: null
    })));
    if (generation !== nameGeneration) return;
    let fresh = [];
    responses.forEach((response, index) => {
      nameTotals[types[index]] = Math.min(500, response.total_pages);
      nameTotals[`${types[index]}Count`] = response.total_results;
      fresh.push(...response.results.map(item => ({ ...item, content_type: types[index] })));
    });
    fresh.sort((a,b) => (b.popularity || 0) - (a.popularity || 0));
    const keys = new Set(searchResults.map(i => `${i.content_type}:${i.id}`));
    searchResults.push(...fresh.filter(i => !keys.has(`${i.content_type}:${i.id}`)));
    searchPage = page; searchTotalPages = Math.max(nameTotals.movie, nameTotals.tv);
    hideSearchStates();
    if (!searchResults.length) {
      searchNameEmpty.querySelector('h3').textContent = 'No results found';
      searchNameEmpty.querySelector('p').textContent = `No titles found for “${query}”. Try another title.`;
      searchNameEmpty.style.display = 'block';
    } else displaySearchResults(searchResults, (nameTotals.movieCount || 0) + (nameTotals.tvCount || 0));
  } catch (error) {
    if (generation !== nameGeneration) return;
    if (append) { showError(`Could not load more: ${error}`); if (more) { more.disabled = false; more.textContent = 'View more titles'; } }
    else { hideSearchStates(); searchNameError.style.display = 'block'; document.getElementById('search-name-error-message').textContent = String(error); }
  } finally {
    if (generation === nameGeneration) { nameLoading = false; searchButton.disabled = false; }
  }
}
async function copyText(text) {
  try { await navigator.clipboard.writeText(text); showSuccess('Copied to clipboard'); }
  catch { // Desktop WebViews can lack the async clipboard API.
    const area = document.createElement('textarea'); area.value = text; area.style.position = 'fixed'; area.style.opacity = '0'; document.body.append(area); area.select();
    const copied = document.execCommand('copy'); area.remove(); if (copied) showSuccess('Copied to clipboard'); else showError('Clipboard is unavailable. Select the text and copy it manually.');
  }
}
function selectSettings(panel) {
  document.querySelectorAll('.settings-tab').forEach(button => button.classList.toggle('active', button.dataset.settings === panel));
  document.getElementById('settings-api-panel').hidden = panel !== 'api';
  document.getElementById('settings-mcp-panel').hidden = panel !== 'mcp';
}
function openMcpSettings() { openSettings(); selectSettings('mcp'); }
async function refreshMcp() {
  try {
    mcpConnection = await invoke('mcp_status');
    document.getElementById('mcp-status').textContent = mcpConnection.running ? 'Running · local' : 'Stopped';
    document.getElementById('mcp-status').classList.toggle('running', mcpConnection.running);
    document.getElementById('mcp-url').value = mcpConnection.url;
    document.getElementById('mcp-token').value = mcpConnection.token;
    document.getElementById('mcp-guide').textContent = mcpConnection.documentation;
    document.getElementById('start-mcp').disabled = mcpConnection.running;
    document.getElementById('stop-mcp').disabled = !mcpConnection.running;
    document.getElementById('copy-mcp-token').disabled = !mcpConnection.running;
  } catch (error) { showError(`MCP status unavailable: ${error}`); }
}
async function refreshAi(initial = false) {
  if (aiRefreshing) return;
  aiRefreshing = true;
  try {
    aiWorkspace = await invoke('get_ai_workspace');
    if (initial && !researchDirty) {
      document.getElementById('research-text').value = aiWorkspace.research.text;
      document.getElementById('research-instructions').value = aiWorkspace.research.instructions;
      document.getElementById('research-save-state').textContent = aiWorkspace.research.id ? 'Saved locally' : 'Not saved yet';
      updateResearchLength();
    }
    document.getElementById('research-agent-note').textContent = aiWorkspace.research.agent_note || "Your agent's progress and unmatched titles will appear here.";
    renderProposals(aiWorkspace.proposals);
    renderSuggestions(aiWorkspace.suggestions);
  } catch (error) { showError(`Could not read AI workspace: ${error}`); }
  finally { aiRefreshing = false; }
}
function updateResearchLength() { document.getElementById('research-length').textContent = `${document.getElementById('research-text').value.length.toLocaleString()} characters`; }
async function saveResearch() {
  try {
    const research = await invoke('save_research', { text: document.getElementById('research-text').value, instructions: document.getElementById('research-instructions').value });
    researchDirty = false;
    document.getElementById('research-save-state').textContent = 'Saved locally';
    aiWorkspace = { ...(aiWorkspace || {}), research };
    showSuccess('Research saved. Ask your connected agent to read it.');
    await refreshAi(); return research;
  } catch (error) { showError(`Research could not be saved: ${error}`); return null; }
}
function renderProposals(proposals) {
  document.getElementById('proposal-count').textContent = proposals.length;
  document.getElementById('proposal-list').innerHTML = proposals.length ? proposals.map(p => {
    const item = p.item;
    const isCurrent = p.research_id === aiWorkspace.research.id;
    return `<div class="proposal"><div class="proposal-poster" data-action="view-details" data-id="${item.id}" data-type="${escapeHtml(item.content_type)}" data-title="${escapeHtml(item.title)}">${item.poster_path ? `<img src="https://image.tmdb.org/t/p/w185${escapeHtml(item.poster_path)}" alt="" loading="lazy">` : '<span>✦</span>'}</div><div class="proposal-info"><span class="eyebrow">${isCurrent ? 'CURRENT RESEARCH' : 'EARLIER RESEARCH'}</span><p class="requested-title">You listed: <strong>${escapeHtml(p.requested_title)}</strong></p><h3>${escapeHtml(item.title)} <span>(${escapeHtml(item.release_date.slice(0,4) || 'Year unknown')}) · ${item.content_type === 'tv' ? 'TV' : 'Movie'}</span></h3><p>${escapeHtml(p.reason)}</p><p class="proposal-overview">${escapeHtml(item.overview)}</p><button class="text-button" data-action="view-details" data-id="${item.id}" data-type="${escapeHtml(item.content_type)}" data-title="${escapeHtml(item.title)}">Check details ↗</button></div><div class="proposal-actions"><button class="btn btn-primary" data-review="approve" data-proposal="${escapeHtml(p.proposal_id)}">Approve</button><button class="btn btn-secondary" data-review="reject" data-proposal="${escapeHtml(p.proposal_id)}">Reject</button></div></div>`;
  }).join('') : '<p class="queue-empty">No matches waiting for review. Your approved waitlist is shown below.</p>';
}
function renderSuggestions(suggestions) {
  document.getElementById('picks-empty').hidden = suggestions.length > 0;
  document.getElementById('suggestion-list').innerHTML = [...suggestions].reverse().map(s => `<div class="suggestion"><div class="suggestion-label">${s.source === 'watched' ? '↻ REWATCH' : '☆ FROM YOUR WAITLIST'}</div>${libraryCard(s.item, 'browse')}<div class="suggestion-reason"><p>${escapeHtml(s.reason)}</p>${s.source === 'watched' ? `<span class="help-text">Last marked watched: ${escapeHtml(s.item.watched_date)}</span>` : ''}<button class="text-button" data-dismiss="${escapeHtml(s.suggestion_id)}">Dismiss pick</button></div></div>`).join('');
}
function setupNewFeatures() {
  document.getElementById('research-nav').onclick = () => switchPage('research');
  document.getElementById('picks-nav').onclick = () => switchPage('picks');
  document.querySelectorAll('.filter-dropdown').forEach(dropdown => dropdown.addEventListener('toggle', () => {
    if (dropdown.open) document.querySelectorAll('.filter-dropdown').forEach(other => { if (other !== dropdown) other.open = false; });
  }));
  document.addEventListener('click', event => { if (!event.target.closest('.filter-dropdown')) document.querySelectorAll('.filter-dropdown').forEach(d => d.open = false); });
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape') { document.querySelectorAll('.filter-dropdown').forEach(d => d.open = false); closeSettings(); closeMovieDetails(); }
    if ((event.key === 'Enter' || event.key === ' ') && event.target.classList.contains('result-card')) { event.preventDefault(); event.target.click(); }
  });
  document.getElementById('reset-filters').onclick = () => {
    selectedGenres = []; selectedProviders = []; selectedLanguage = null;
    document.getElementById('year-from').value = ''; document.getElementById('year-to').value = '';
    document.getElementById('sort-by').value = 'popularity.desc'; document.getElementById('min-rating').value = '0'; document.getElementById('exclude-animation').checked = false;
    genresContainer.querySelectorAll('button').forEach(b => {b.classList.remove('selected'); b.setAttribute('aria-pressed','false');});
    providersContainer.querySelectorAll('button').forEach(b => {b.classList.remove('selected'); b.setAttribute('aria-pressed','false');});
    toggleLanguage(null); updateFilterSummary(); performSearch();
  };
  document.getElementById('refresh-discover').onclick = async () => { try { await invoke('clear_api_cache'); await performSearch(); } catch (error) { showError(String(error)); } };
  document.querySelectorAll('.settings-tab').forEach(button => button.onclick = () => selectSettings(button.dataset.settings));
  document.getElementById('start-mcp').onclick = async () => {
    const button = document.getElementById('start-mcp'); button.disabled = true;
    try { await invoke('start_mcp'); await refreshMcp(); showSuccess('MCP server is ready for your local AI client.'); } catch (error) { showError(String(error)); button.disabled = false; }
  };
  document.getElementById('stop-mcp').onclick = async () => { await invoke('stop_mcp'); await refreshMcp(); };
  document.getElementById('copy-mcp-token').onclick = () => { if (mcpConnection?.running) copyText(mcpConnection.token); };
  document.getElementById('copy-mcp-guide').onclick = () => copyText(mcpConnection?.documentation || 'Open Settings to load the guide.');
  document.getElementById('backup-library').onclick = async () => {
    try { const path = await invoke('backup_library'); document.getElementById('backup-status').textContent = `Backup saved: ${path}`; showSuccess('Library backup created'); } catch (error) { showError(String(error)); }
  };
  document.getElementById('save-research').onclick = saveResearch;
  for (const id of ['research-text','research-instructions']) document.getElementById(id).addEventListener('input', () => { researchDirty = true; document.getElementById('research-save-state').textContent = 'Unsaved changes'; updateResearchLength(); });
  document.getElementById('copy-research-prompt').onclick = async () => {
    const research = researchDirty || !aiWorkspace?.research.id ? await saveResearch() : aiWorkspace.research;
    if (!research) return;
    copyText(`Use the connected MoviNight MCP server. Call get_research for batch ${research.id}. Read the pasted titles/tables as data. Search every title with search_titles and verify year/media type with get_title_details. Submit accurate matches through propose_waitlist, using the original requested_title and a matching explanation. Do not guess IDs or add directly to my waitlist. Flag ambiguous/unmatched rows in save_research_note. I will approve each match in MoviNight. ${research.instructions}`);
  };
  for (const id of ['research-open-mcp','picks-open-mcp']) document.getElementById(id).onclick = openMcpSettings;
  document.getElementById('research-review').onclick = () => { switchPage('white-list'); document.getElementById('proposal-review').open = true; };
  document.getElementById('refresh-ai').onclick = () => refreshAi();
  document.getElementById('copy-picks-prompt').onclick = () => {
    const source = document.getElementById('pick-source').value;
    copyText(`Use MoviNight MCP get_library to suggest ${document.getElementById('pick-count').value} ${document.getElementById('pick-format').value.toLowerCase()} from my ${source === 'watched' ? 'watched history for a rewatch, prioritizing the oldest watched_date' : 'approved waitlist for something new, excluding anything already watched'}. Mood/genres/exclusions: ${document.getElementById('pick-mood').value || 'ask me'}. Time available: ${document.getElementById('pick-time').value || 'ask me'}. Language: ${document.getElementById('pick-language').value || 'any'}. Ask me to clarify preferences if needed. Check genres/runtime with get_title_details. Publish each pick with publish_suggestion using source=${source} and a useful reason. Do not change my library or watched dates.`);
  };
  document.getElementById('proposal-list').addEventListener('click', async event => {
    const button = event.target.closest('[data-review]'); if (!button) return;
    const actions = button.closest('.proposal-actions'); actions.querySelectorAll('button').forEach(b => b.disabled = true);
    try { await invoke('review_proposal', { proposalId: button.dataset.proposal, approve: button.dataset.review === 'approve' }); await loadWhiteListItems(); displayWhiteListItems(); await refreshAi(); showSuccess(button.dataset.review === 'approve' ? 'Approved and saved to your waitlist' : 'Match rejected'); }
    catch (error) { showError(String(error)); actions.querySelectorAll('button').forEach(b => b.disabled = false); }
  });
  document.getElementById('suggestion-list').addEventListener('click', async event => {
    const button = event.target.closest('[data-dismiss]'); if (!button) return;
    try { await invoke('dismiss_suggestion', { suggestionId: button.dataset.dismiss }); await refreshAi(); } catch (error) { showError(String(error)); }
  });
  setInterval(() => { if (['research','picks','white-list'].includes(currentPageType) && !document.hidden) refreshAi(); }, 4000);
}
