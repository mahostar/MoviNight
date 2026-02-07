![MoviNight](branding/logoGit.png)

A modern, fast, and beautiful movie and TV show discovery application built with Tauri v2.

![Demo animation](branding/demo.gif)

## ✨ Features

- **🔍 Advanced Discovery**: Filter movies and TV shows by genre, year, language, and streaming providers
- **🔎 Smart Search**: Search by title across both movies and TV shows simultaneously  
- **📋 Watchlist Management**: Keep track of what you want to watch and what you've already seen
- **🎬 Trailer Integration**: Watch trailers directly in the app with embedded YouTube player
- **📱 Responsive Design**: Beautiful UI that works on all screen sizes
- **⚡ Fast & Lightweight**: Built with Rust backend for optimal performance
- **🔒 Secure**: Your data stays local - no cloud sync required
- **🌐 Cross-Platform**: Works on Windows, macOS, and Linux

### Development

1. **Clone the repository**
   ```bash
   git clone https://github.com/mahostar/movinight
   cd movinight
   ```

2. **Install dependencies**
   ```bash
   npm install
   ```

3. **Start development server**
   ```bash
   cargo tauri dev
   ```

### Building for Production

Build the application for your platform:

```bash
cargo tauri build
```

Cleaning cache:
```bash
cd src-tauri
```
```bash
cargo clean
```
```bash
cd ..
```
```bash
cargo tauri dev
```

This will create platform-specific installers in `src-tauri/target/release/bundle/`.

## 🛠️ Tech Stack

- **Frontend**: HTML5, CSS3, Vanilla JavaScript
- **Backend**: Rust with Tauri v2
- **API**: The Movie Database (TMDB) API
- **HTTP Client**: reqwest
- **Data Storage**: Local JSON files
- **UI Framework**: Custom responsive design

## 📱 Supported Platforms

- ✅ Windows 7+
- ✅ macOS 10.15+
- ✅ Linux (most distributions)

## 🔧 Configuration

1. **Get TMDB API Key**
   - Visit [TMDB API Settings](https://www.themoviedb.org/settings/api)
   - Create a free account and request an API key

2. **Set API Key**
   - Open MoviNight
   - Click the settings gear icon (⚙️)
   - Enter your TMDB API key
   - Click Save

## 📁 Project Structure

```
MoviNight/
├── src-tauri/          # Rust backend
│   ├── src/
│   │   ├── lib.rs      # Main application logic
│   │   └── main.rs     # Entry point
│   ├── icons/          # App icons
│   ├── Cargo.toml      # Rust dependencies
│   └── tauri.conf.json # Tauri configuration
├── index.html          # Main HTML file
├── main.js            # JavaScript application logic
├── styles.css         # CSS styles
├── logo.png           # Application logo
└── README.md          # This file
```

## 🎯 Usage

1. **Discovery**: Browse trending and popular content with advanced filters
2. **Search**: Find specific movies or TV shows by name
3. **Watchlist**: Add items to your white list for later viewing
4. **Tracking**: Mark content as watched to keep track of your viewing history
5. **Details**: Click any item to see full details, trailers, and metadata

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [The Movie Database (TMDB)](https://www.themoviedb.org/) for providing the API
- [Tauri](https://tauri.app/) for the amazing framework
- [Rust](https://www.rust-lang.org/) for the powerful backend

## 📞 Support

If you have any questions or need help, please open an issue on GitHub.

---

**Built with ❤️ by Mahostar** 
