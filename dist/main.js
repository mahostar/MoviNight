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
  
  // Disable always on top after 1 second (so it pops up initially then behaves normally)
  setTimeout(async () => {
    // In Tauri v2, window management functions are in the `window` namespace.
    if (window.__TAURI__?.window) {
      const { getCurrent } = window.__TAURI__.window;
      const currentWindow = getCurrent();
      // This ensures the window is on top for 1 sec, then behaves normally.
      await currentWindow.setAlwaysOnTop(false);
    }
  }, 1000);
  
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
  // Start splash screen
  initializeSplashScreen();
  
  initializeElements();
  setupEventListeners();
  await loadApiKey();
  await loadWatchedItems();
  await loadWhiteListItems();
  await loadGenres();
  await loadLanguages();
  await loadWatchProviders();
  
  // Initialize year range with dynamic current year
  initializeYearRange();
  
  // Set default sort order to newest first
  document.getElementById('sort-by').value = 'release_date.desc';
  
  // Trigger initial search with default values
  performSearch();
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

  // Movie details modal
  document.getElementById('close-movie-details').addEventListener('click', closeMovieDetails);

  // Navigation
  document.getElementById('discover-nav').addEventListener('click', () => switchPage('discover'));
  document.getElementById('search-nav').addEventListener('click', () => switchPage('search'));
  document.getElementById('watched-nav').addEventListener('click', () => switchPage('watched'));
  document.getElementById('white-list-nav').addEventListener('click', () => switchPage('white-list'));
  document.getElementById('go-to-discover').addEventListener('click', () => switchPage('discover'));

  // Content type toggle
  document.querySelectorAll('.toggle-btn').forEach(btn => {
    btn.addEventListener('click', (e) => {
      const type = e.target.dataset.type;
      if (type && type !== currentContentType) {
        switchContentType(type);
      }
    });
  });

  // Watched filter toggle
  document.querySelectorAll('[data-filter]').forEach(btn => {
    btn.addEventListener('click', (e) => {
      const filter = e.target.dataset.filter;
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
        // Move from white list to watched
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
      showError('Failed to process request. Please try again.');
    }
  });
}

function switchPage(pageType) {
  currentPageType = pageType;
  
  // Update navigation buttons
  document.querySelectorAll('.nav-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.page === pageType);
  });

  // Show/hide page content
  document.getElementById('discover-page').style.display = pageType === 'discover' ? 'block' : 'none';
  document.getElementById('search-page').style.display = pageType === 'search' ? 'block' : 'none';
  document.getElementById('watched-page').style.display = pageType === 'watched' ? 'block' : 'none';
  document.getElementById('white-list-page').style.display = pageType === 'white-list' ? 'block' : 'none';

  if (pageType === 'watched') {
    displayWatchedItems();
  } else if (pageType === 'white-list') {
    displayWhiteListItems();
  } else if (pageType === 'search') {
    // Focus search input when switching to search page
    setTimeout(() => {
      searchInput.focus();
    }, 100);
  }
}

async function loadWatchedItems() {
  try {
    watchedItems = await invoke('get_watched_items');
    updateWatchedStats();
  } catch (error) {
    console.error('Failed to load watched items:', error);
    watchedItems = [];
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

function renderWatchedCard(item) {
  const year = item.release_date ? new Date(item.release_date).getFullYear() : 'N/A';
  const rating = (item.vote_average !== null && item.vote_average !== undefined) ? item.vote_average.toFixed(1) : 'N/A';
  const overview = item.overview || 'No overview available.';
  const watchedDate = item.watched_date ? new Date(item.watched_date).toLocaleDateString() : '';
  
  const posterUrl = item.poster_path 
    ? `https://image.tmdb.org/t/p/w500${item.poster_path}`
    : 'data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgZmlsbD0iI2UyZThmMCIvPjx0ZXh0IHg9IjUwJSIgeT0iNTAlIiBmb250LWZhbWlseT0iQXJpYWwsIHNhbnMtc2VyaWYiIGZvbnQtc2l6ZT0iMjQiIGZpbGw9IiM3MTgwOTYiIHRleHQtYW5jaG9yPSJtaWRkbGUiIGR5PSIwLjNlbSI+SW1hZ2UgTm90IEF2YWlsYWJsZTwvdGV4dD48L3N2Zz4=';

  return `
    <div class="result-card" data-action="view-details" data-id="${item.id}" data-type="${item.content_type}" data-title="${item.title}">
      <button class="remove-btn" data-action="remove-watched" data-id="${item.id}" data-type="${item.content_type}" title="Remove from watched">
        ❌
      </button>
      <img 
        src="${posterUrl}" 
        alt="${item.title}"
        class="result-poster"
        loading="lazy"
        onerror="this.src='data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgZmlsbD0iI2UyZThmMCIvPjx0ZXh0IHg9IjUwJSIgeT0iNTAlIiBmb250LWZhbWlseT0iQXJpYWwsIHNhbnMtc2VyaWYiIGZvbnQtc2l6ZT0iMjQiIGZpbGw9IiM3MTgwOTYiIHRleHQtYW5jaG9yPSJtaWRkbGUiIGR5PSIwLjNlbSI+SW1hZ2UgTm90IEF2YWlsYWJsZTwvdGV4dD48L3N2Zz4='; this.onerror=null;"
      />
      <div class="result-info">
        <div class="result-title">${item.title}</div>
        <div class="result-meta">
          <span class="result-year">${year}</span>
          <span class="result-rating">⭐ ${rating}</span>
          <span class="result-type">${item.content_type === 'movie' ? '🎬' : '📺'}</span>
        </div>
        <div class="result-overview">${overview}</div>
        <div class="watched-date">Watched: ${watchedDate}</div>
      </div>
    </div>
  `;
}

// White List Functions (duplicated from watched)
async function loadWhiteListItems() {
  try {
    whiteListItems = await invoke('get_white_list_items');
    updateWhiteListStats();
  } catch (error) {
    console.error('Failed to load white list items:', error);
    whiteListItems = [];
  }
}

function updateWhiteListStats() {
  const movieCount = whiteListItems.filter(item => item.content_type === 'movie').length;
  const tvCount = whiteListItems.filter(item => item.content_type === 'tv').length;
  const totalCount = whiteListItems.length;
  
  if (whiteListStats) {
    whiteListStats.textContent = `${totalCount} items white listed (${movieCount} movies, ${tvCount} TV shows)`;
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

function renderWhiteListCard(item) {
  const year = item.release_date ? new Date(item.release_date).getFullYear() : 'N/A';
  const rating = (item.vote_average !== null && item.vote_average !== undefined) ? item.vote_average.toFixed(1) : 'N/A';
  const overview = item.overview || 'No overview available.';
  const whiteListDate = item.white_list_date ? new Date(item.white_list_date).toLocaleDateString() : '';
  
  const posterUrl = item.poster_path 
    ? `https://image.tmdb.org/t/p/w500${item.poster_path}`
    : 'data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgZmlsbD0iI2UyZThmMCIvPjx0ZXh0IHg9IjUwJSIgeT0iNTAlIiBmb250LWZhbWlseT0iQXJpYWwsIHNhbnMtc2VyaWYiIGZvbnQtc2l6ZT0iMjQiIGZpbGw9IiM3MTgwOTYiIHRleHQtYW5jaG9yPSJtaWRkbGUiIGR5PSIwLjNlbSI+SW1hZ2UgTm90IEF2YWlsYWJsZTwvdGV4dD48L3N2Zz4=';

  return `
    <div class="result-card" data-action="view-details" data-id="${item.id}" data-type="${item.content_type}" data-title="${item.title}">
      <button class="remove-btn" data-action="remove-whitelist" data-id="${item.id}" data-type="${item.content_type}" title="Remove from white list">
        ❌
      </button>
      <button class="watch-btn" data-action="watch-from-whitelist" data-id="${item.id}" data-type="${item.content_type}" data-title="${item.title.replace(/"/g, '&quot;')}" data-overview="${overview.replace(/"/g, '&quot;').replace(/\n/g, ' ')}" data-poster-path="${item.poster_path || ''}" data-date="${item.release_date || ''}" data-vote-average="${item.vote_average || 0}" title="Mark as watched">
        👁️
      </button>
      <img 
        src="${posterUrl}" 
        alt="${item.title}"
        class="result-poster"
        loading="lazy"
        onerror="this.src='data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgZmlsbD0iI2UyZThmMCIvPjx0ZXh0IHg9IjUwJSIgeT0iNTAlIiBmb250LWZhbWlseT0iQXJpYWwsIHNhbnMtc2VyaWYiIGZvbnQtc2l6ZT0iMjQiIGZpbGw9IiM3MTgwOTYiIHRleHQtYW5jaG9yPSJtaWRkbGUiIGR5PSIwLjNlbSI+SW1hZ2UgTm90IEF2YWlsYWJsZTwvdGV4dD48L3N2Zz4='; this.onerror=null;"
      />
      <div class="result-info">
        <div class="result-title">${item.title}</div>
        <div class="result-meta">
          <span class="result-year">${year}</span>
          <span class="result-rating">⭐ ${rating}</span>
          <span class="result-type">${item.content_type === 'movie' ? '🎬' : '📺'}</span>
        </div>
        <div class="result-overview">${overview}</div>
        <div class="white-list-date">White Listed: ${whiteListDate}</div>
      </div>
    </div>
  `;
}

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
    
    showSuccess(`Added "${title}" to white list!`);
  } catch (error) {
    showError(`Failed to add to white list: ${error}`);
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
    
    showSuccess('Removed from white list!');
  } catch (error) {
    showError(`Failed to remove from white list: ${error}`);
  }
}

function updateWhiteListButton(id, contentType, isWhiteListed) {
  const buttons = document.querySelectorAll(`[data-action="whitelist"][data-id="${id}"][data-type="${contentType}"]`);
  buttons.forEach(button => {
    if (isWhiteListed) {
      button.classList.add('white-listed');
      button.textContent = '⭐';
      button.title = 'Remove from white list';
    } else {
      button.classList.remove('white-listed');
      button.textContent = '☆';
      button.title = 'Add to white list';
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
    await loadGenres(); // Reload genres with new API key
    showSuccess('API key saved successfully!');
  } catch (error) {
    showError(`Failed to save API key: ${error}`);
  }
}

function openSettings() {
  settingsModal.classList.add('active');
  apiKeyInput.focus();
}

function closeSettings() {
  settingsModal.classList.remove('active');
}

async function switchContentType(type) {
  currentContentType = type;
  selectedGenres = [];
  selectedProviders = [];
  // Note: selectedLanguage is kept across content types
  
  // Update toggle buttons
  document.querySelectorAll('.toggle-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.type === type);
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

  await loadGenres();
  await loadWatchProviders();
  clearResults();
}

async function loadGenres() {
  try {
    genresContainer.innerHTML = '<div class="loading-text">Loading genres...</div>';
    
    const genres = currentContentType === 'movie' 
      ? await invoke('get_movie_genres')
      : await invoke('get_tv_genres');
    
    allGenres[currentContentType] = genres;
    renderGenres(genres);
  } catch (error) {
    genresContainer.innerHTML = `<div class="error-text">Failed to load genres: ${error}</div>`;
  }
}

function renderGenres(genres) {
  genresContainer.innerHTML = genres.map(genre => 
    `<div class="genre-chip" data-id="${genre.id}">${genre.name}</div>`
  ).join('');

  // Add click handlers to genre chips
  genresContainer.querySelectorAll('.genre-chip').forEach(chip => {
    chip.addEventListener('click', () => toggleGenre(parseInt(chip.dataset.id)));
  });
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
  });
}

async function loadLanguages() {
  try {
    languageContainer.innerHTML = '<div class="loading-text">Loading languages...</div>';
    const languages = await invoke('get_languages');
    // We only want to show a curated list of common languages
    const commonLanguages = ["en", "es", "fr", "de", "it", "ja", "ko", "ru", "zh", "hi"];
    const filteredLanguages = languages.filter(lang => commonLanguages.includes(lang.iso_639_1));

    languageContainer.innerHTML = filteredLanguages.map(lang => 
      `<div class="language-chip" data-id="${lang.iso_639_1}">${lang.english_name}</div>`
    ).join('');

    languageContainer.querySelectorAll('.language-chip').forEach(chip => {
      chip.addEventListener('click', () => toggleLanguage(chip.dataset.id));
    });
  } catch (error) {
    languageContainer.innerHTML = `<div class="error-text">Failed to load languages: ${error}</div>`;
  }
}

function toggleLanguage(langCode) {
  if (selectedLanguage === langCode) {
    selectedLanguage = null; // Deselect
  } else {
    selectedLanguage = langCode; // Select
  }

  languageContainer.querySelectorAll('.language-chip').forEach(chip => {
    chip.classList.toggle('selected', chip.dataset.id === selectedLanguage);
  });
}

async function loadWatchProviders() {
  try {
    providersContainer.innerHTML = '<div class="loading-text">Loading providers...</div>';
    const providers = await invoke('get_watch_providers', { contentType: currentContentType });
    // Filter for some popular providers to keep the list manageable
    const popularProviderIds = [8, 9, 15, 337, 1899, 531, 384, 2, 350]; // Netflix, Amazon Prime, Hulu, Disney+, Max, Paramount+, Apple TV, Apple TV+
    const filteredProviders = providers.filter(p => popularProviderIds.includes(p.provider_id));

    providersContainer.innerHTML = filteredProviders.map(provider => 
      `<div class="provider-chip" data-id="${provider.provider_id}">
         <img src="https://image.tmdb.org/t/p/w92${provider.logo_path}" alt="${provider.provider_name} logo">
         ${provider.provider_name}
       </div>`
    ).join('');

    providersContainer.querySelectorAll('.provider-chip').forEach(chip => {
      chip.addEventListener('click', () => toggleProvider(parseInt(chip.dataset.id)));
    });
  } catch (error) {
    providersContainer.innerHTML = `<div class="error-text">Failed to load providers: ${error}</div>`;
  }
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
  });
}

async function performSearch() {
  const yearFromInput = document.getElementById('year-from').value;
  const yearToInput = document.getElementById('year-to').value;
  const sortBy = document.getElementById('sort-by').value;
  const excludeAnimation = document.getElementById('exclude-animation').checked;

  // Parse year values - left side is START year (from), right side is END year (to)
  let yearFrom = parseInt(yearFromInput) || 1980;
  let yearTo = parseInt(yearToInput) || new Date().getFullYear();

  // Ensure yearFrom is always less than or equal to yearTo
  if (yearFrom > yearTo) {
    [yearFrom, yearTo] = [yearTo, yearFrom];
  }

  currentFilters = {
    yearFrom: yearFrom,
    yearTo: yearTo,
    sortBy: sortBy,
    genreIds: selectedGenres,
    excludeAnimation: excludeAnimation,
    watchProviders: selectedProviders,
    originalLanguage: selectedLanguage
  };

  // Reset for new search
  currentPage = 1;
  allResults = [];
  await searchContent();
}

async function loadMoreResults() {
  if (currentPage < totalPages) {
    currentPage++;
    await searchContent(true); // true means append to existing results
  }
}

async function searchContent(appendResults = false) {
  try {
    if (!appendResults) {
    showLoading();
    } else {
      // Show loading state on the view more button
      const viewMoreBtn = document.getElementById('view-more-btn');
      viewMoreBtn.textContent = '⏳ Loading...';
      viewMoreBtn.disabled = true;
    }
    
    const response = currentContentType === 'movie'
      ? await invoke('search_movies', {
          query: '',
          page: currentPage,
          yearFrom: currentFilters.yearFrom,
          yearTo: currentFilters.yearTo,
          genreIds: currentFilters.genreIds,
          sortBy: currentFilters.sortBy,
          excludeAnimation: currentFilters.excludeAnimation,
          withWatchProviders: currentFilters.watchProviders,
          withOriginalLanguage: currentFilters.originalLanguage
        })
      : await invoke('search_tv_shows', {
          query: '',
          page: currentPage,
          yearFrom: currentFilters.yearFrom,
          yearTo: currentFilters.yearTo,
          genreIds: currentFilters.genreIds,
          sortBy: currentFilters.sortBy,
          excludeAnimation: currentFilters.excludeAnimation,
          withWatchProviders: currentFilters.watchProviders,
          withOriginalLanguage: currentFilters.originalLanguage
        });
    
    if (appendResults) {
      // Add new results to existing array
      allResults = [...allResults, ...response.results];
      // Create a response object with all results for display
      const combinedResponse = {
        ...response,
        results: allResults
      };
      displayResults(combinedResponse, true, response.results);
    } else {
      allResults = response.results;
      totalPages = response.total_pages;
    displayResults(response);
    }
  } catch (error) {
    showError(`Search failed: ${error}`);
    // Reset view more button on error
    if (appendResults) {
      const viewMoreBtn = document.getElementById('view-more-btn');
      viewMoreBtn.textContent = '📺 View More Results';
      viewMoreBtn.disabled = false;
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

function showError(message) {
  hideAllStates();
  errorMessage.textContent = message;
  errorState.style.display = 'block';
}

function showSuccess(message) {
  console.log('✅', message);
  
  // Show a temporary success message
  const successMsg = document.createElement('div');
  successMsg.textContent = message;
  successMsg.style.cssText = `
    position: fixed;
    top: 20px;
    right: 20px;
    background: var(--success);
    color: white;
    padding: 1rem 1.5rem;
    border-radius: 8px;
    z-index: 10000;
    animation: slideIn 0.3s ease;
  `;
  
  document.body.appendChild(successMsg);
  setTimeout(() => {
    successMsg.remove();
  }, 3000);
}

async function displayResults(response, isAppending = false, newResults = []) {
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
    const newResultsHtml = await Promise.all(newResults.map(item => renderResultCard(item)));
    resultsGrid.innerHTML += newResultsHtml.join('');
  } else {
    // Replace all results
    const resultsHtml = await Promise.all(response.results.map(item => renderResultCard(item)));
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

  // Scroll to results on first load
  if (!isAppending) {
  resultsHeader.scrollIntoView({ behavior: 'smooth' });
  }
}

async function renderResultCard(item) {
  const title = item.title || item.name;
  const date = item.release_date || item.first_air_date;
  const year = date ? new Date(date).getFullYear() : 'N/A';
  const rating = (item.vote_average !== null && item.vote_average !== undefined) ? item.vote_average.toFixed(1) : 'N/A';
  const overview = item.overview || 'No overview available.';
  
  // Only use TMDB images if poster_path exists, otherwise show placeholder immediately
  const posterUrl = item.poster_path 
    ? `https://image.tmdb.org/t/p/w500${item.poster_path}`
    : 'data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgZmlsbD0iI2UyZThmMCIvPjx0ZXh0IHg9IjUwJSIgeT0iNTAlIiBmb250LWZhbWlseT0iQXJpYWwsIHNhbnMtc2VyaWYiIGZvbnQtc2l6ZT0iMjQiIGZpbGw9IiM3MTgwOTYiIHRleHQtYW5jaG9yPSJtaWRkbGUiIGR5PSIwLjNlbSI+SW1hZ2UgTm90IEF2YWlsYWJsZTwvdGV4dD48L3N2Zz4=';

  // Check if item is already watched
  const isWatched = watchedItems.some(watchedItem => 
    watchedItem.id === item.id && watchedItem.content_type === currentContentType
  );

  // Check if item is already white listed
  const isWhiteListed = whiteListItems.some(whiteListItem => 
    whiteListItem.id === item.id && whiteListItem.content_type === currentContentType
  );

  const watchButtonClass = isWatched ? 'watch-btn watched' : 'watch-btn';
  const watchButtonText = isWatched ? '✅' : '👁️';
  const watchButtonTitle = isWatched ? 'Remove from watched' : 'Mark as watched';

  const whiteListButtonClass = isWhiteListed ? 'white-list-btn white-listed' : 'white-list-btn';
  const whiteListButtonText = isWhiteListed ? '⭐' : '☆';
  const whiteListButtonTitle = isWhiteListed ? 'Remove from white list' : 'Add to white list';

  return `
    <div class="result-card" data-action="view-details" data-id="${item.id}" data-type="${currentContentType}" data-title="${title}">
      <button 
        class="${watchButtonClass}" 
        data-action="watch"
        data-id="${item.id}" 
        data-type="${currentContentType}"
        data-title="${title.replace(/"/g, '&quot;')}"
        data-overview="${overview.replace(/"/g, '&quot;').replace(/\n/g, ' ')}"
        data-poster-path="${item.poster_path || ''}"
        data-date="${date || ''}"
        data-vote-average="${item.vote_average || 0}"
        title="${watchButtonTitle}"
      >
        ${watchButtonText}
      </button>
      <button 
        class="${whiteListButtonClass}" 
        data-action="whitelist"
        data-id="${item.id}" 
        data-type="${currentContentType}"
        data-title="${title.replace(/"/g, '&quot;')}"
        data-overview="${overview.replace(/"/g, '&quot;').replace(/\n/g, ' ')}"
        data-poster-path="${item.poster_path || ''}"
        data-date="${date || ''}"
        data-vote-average="${item.vote_average || 0}"
        title="${whiteListButtonTitle}"
      >
        ${whiteListButtonText}
      </button>
      <img 
        src="${posterUrl}" 
        alt="${title}"
        class="result-poster"
        loading="lazy"
        onerror="this.src='data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgZmlsbD0iI2UyZThmMCIvPjx0ZXh0IHg9IjUwJSIgeT0iNTAlIiBmb250LWZhbWlseT0iQXJpYWwsIHNhbnMtc2VyaWYiIGZvbnQtc2l6ZT0iMjQiIGZpbGw9IiM3MTgwOTYiIHRleHQtYW5jaG9yPSJtaWRkbGUiIGR5PSIwLjNlbSI+SW1hZ2UgTm90IEF2YWlsYWJsZTwvdGV4dD48L3N2Zz4='; this.onerror=null;"
      />
      <div class="result-info">
        <div class="result-title">${title}</div>
        <div class="result-meta">
          <span class="result-year">${year}</span>
          <span class="result-rating">⭐ ${rating}</span>
        </div>
        <div class="result-overview">${overview}</div>
      </div>
    </div>
  `;
}

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
}

function clearSearch() {
  searchInput.value = '';
  searchClearBtn.classList.remove('visible');
  searchInput.focus();
  clearSearchResults();
}

function clearSearchResults() {
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
  
  if (!query) {
    clearSearchResults();
    return;
  }

  hideSearchStates();
  searchNameLoading.style.display = 'block';
  
  try {
    // Search both movies and TV shows in parallel
    const [movieResponse, tvResponse] = await Promise.all([
      invoke('search_movies', {
        query: query,
        page: 1,
        yearFrom: null,
        yearTo: null,
        genreIds: [],
        sortBy: 'popularity.desc',
        excludeAnimation: false,
        withWatchProviders: [],
        withOriginalLanguage: null
      }),
      invoke('search_tv_shows', {
        query: query,
        page: 1,
        yearFrom: null,
        yearTo: null,
        genreIds: [],
        sortBy: 'popularity.desc',
        excludeAnimation: false,
        withWatchProviders: [],
        withOriginalLanguage: null
      })
    ]);

    // Combine results and sort by popularity
    const combinedResults = [
      ...movieResponse.results.map(item => ({...item, content_type: 'movie'})),
      ...tvResponse.results.map(item => ({...item, content_type: 'tv'}))
    ].sort((a, b) => b.popularity - a.popularity);

    searchResults = combinedResults;
    
    hideSearchStates();
    
    if (combinedResults.length === 0) {
      searchNameEmpty.querySelector('h3').textContent = 'No Results Found';
      searchNameEmpty.querySelector('p').textContent = `No movies or TV shows found for "${query}". Try a different search term.`;
      searchNameEmpty.style.display = 'block';
    } else {
      displaySearchResults(combinedResults, movieResponse.total_results + tvResponse.total_results);
    }
    
  } catch (error) {
    console.error('Search failed:', error);
    hideSearchStates();
    searchNameError.style.display = 'block';
    document.getElementById('search-name-error-message').textContent = error.toString();
  }
}

function displaySearchResults(results, totalResults) {
  const headerHtml = `
    <div class="search-results-header">
      <h3>Search Results</h3>
      <div class="search-results-info">Found ${results.length} results (${totalResults} total across all pages)</div>
    </div>
  `;
  
  const resultsHtml = results.map(item => renderSearchResultCard(item)).join('');
  
  searchNameResults.innerHTML = `
    ${headerHtml}
    <div class="search-results-grid">
      ${resultsHtml}
    </div>
  `;
}

function renderSearchResultCard(item) {
  const title = item.title || item.name;
  const date = item.release_date || item.first_air_date;
  const year = date ? new Date(date).getFullYear() : 'N/A';
  const rating = (item.vote_average !== null && item.vote_average !== undefined) ? item.vote_average.toFixed(1) : 'N/A';
  const overview = item.overview || 'No overview available.';
  
  const posterUrl = item.poster_path 
    ? `https://image.tmdb.org/t/p/w500${item.poster_path}`
    : 'data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgZmlsbD0iI2UyZThmMCIvPjx0ZXh0IHg9IjUwJSIgeT0iNTAlIiBmb250LWZhbWlseT0iQXJpYWwsIHNhbnMtc2VyaWYiIGZvbnQtc2l6ZT0iMjQiIGZpbGw9IiM3MTgwOTYiIHRleHQtYW5jaG9yPSJtaWRkbGUiIGR5PSIwLjNlbSI+SW1hZ2UgTm90IEF2YWlsYWJsZTwvdGV4dD48L3N2Zz4=';

  // Check if item is already watched
  const isWatched = watchedItems.some(watchedItem => 
    watchedItem.id === item.id && watchedItem.content_type === item.content_type
  );

  // Check if item is already white listed
  const isWhiteListed = whiteListItems.some(whiteListItem => 
    whiteListItem.id === item.id && whiteListItem.content_type === item.content_type
  );

  const watchButtonClass = isWatched ? 'watch-btn watched' : 'watch-btn';
  const watchButtonText = isWatched ? '✅' : '👁️';
  const watchButtonTitle = isWatched ? 'Remove from watched' : 'Mark as watched';

  const whiteListButtonClass = isWhiteListed ? 'white-list-btn white-listed' : 'white-list-btn';
  const whiteListButtonText = isWhiteListed ? '⭐' : '☆';
  const whiteListButtonTitle = isWhiteListed ? 'Remove from white list' : 'Add to white list';

  return `
    <div class="result-card" data-action="view-details" data-id="${item.id}" data-type="${item.content_type}" data-title="${title}">
      <button 
        class="${watchButtonClass}" 
        data-action="watch"
        data-id="${item.id}" 
        data-type="${item.content_type}"
        data-title="${title.replace(/"/g, '&quot;')}"
        data-overview="${overview.replace(/"/g, '&quot;').replace(/\n/g, ' ')}"
        data-poster-path="${item.poster_path || ''}"
        data-date="${date || ''}"
        data-vote-average="${item.vote_average || 0}"
        title="${watchButtonTitle}"
      >
        ${watchButtonText}
      </button>
      <button 
        class="${whiteListButtonClass}" 
        data-action="whitelist"
        data-id="${item.id}" 
        data-type="${item.content_type}"
        data-title="${title.replace(/"/g, '&quot;')}"
        data-overview="${overview.replace(/"/g, '&quot;').replace(/\n/g, ' ')}"
        data-poster-path="${item.poster_path || ''}"
        data-date="${date || ''}"
        data-vote-average="${item.vote_average || 0}"
        title="${whiteListButtonTitle}"
      >
        ${whiteListButtonText}
      </button>
      <img 
        src="${posterUrl}" 
        alt="${title}"
        class="result-poster"
        loading="lazy"
        onerror="this.src='data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj48cmVjdCB3aWR0aD0iNTAwIiBoZWlnaHQ9Ijc1MCIgZmlsbD0iI2UyZThmMCIvPjx0ZXh0IHg9IjUwJSIgeT0iNTAlIiBmb250LWZhbWlseT0iQXJpYWwsIHNhbnMtc2VyaWYiIGZvbnQtc2l6ZT0iMjQiIGZpbGw9IiM3MTgwOTYiIHRleHQtYW5jaG9yPSJtaWRkbGUiIGR5PSIwLjNlbSI+SW1hZ2UgTm90IEF2YWlsYWJsZTwvdGV4dD48L3N2Zz4='; this.onerror=null;"
      />
      <div class="result-info">
        <div class="result-title">${title}</div>
        <div class="result-meta">
          <span class="result-year">${year}</span>
          <span class="result-rating">⭐ ${rating}</span>
          <span class="result-type">${item.content_type === 'movie' ? '🎬' : '📺'}</span>
        </div>
        <div class="result-overview">${overview}</div>
      </div>
    </div>
  `;
}

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
  yearFromInput.value = '2015';
  yearToInput.value = currentYear.toString();

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

  // Set modal title and show modal
  modalTitle.textContent = `${contentType === 'movie' ? 'Movie' : 'TV Show'} Details`;
  modal.classList.add('active');

  // Reset states
  loading.style.display = 'block';
  content.style.display = 'none';
  error.style.display = 'none';

  try {
    // Fetch details and trailers in parallel
    const [details, trailers] = await Promise.all([
      contentType === 'movie' 
        ? invoke('get_movie_details', { id })
        : invoke('get_tv_details', { id }),
      invoke('get_trailers', { id, contentType })
    ]);

    console.log('Movie details:', details);
    console.log('Trailers:', trailers);

    // Populate the modal
    populateMovieDetails(details, trailers, contentType);
    
    // Show content
    loading.style.display = 'none';
    content.style.display = 'block';
  } catch (err) {
    console.error('Failed to load movie details:', err);
    loading.style.display = 'none';
    error.style.display = 'block';
    document.getElementById('movie-details-error-message').textContent = err.toString();
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
  rating.textContent = `⭐ ${voteAverage}`;
  
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
      .map(genre => `<span class="movie-genre-chip">${genre.name}</span>`)
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
  
  // Hide modal
  modal.classList.remove('active');
}