Here is the README file in the format used on GitHub:

**Z-Index Mapper**
================

Z-Index Mapper is a powerful web-based tool designed to help developers visualize, analyze, and manage z-index values across their project files. It provides an interactive interface to explore z-index usage, identify potential stacking issues, and maintain better control over layering in web applications.

**Features**
------------

* Real-time Scanning: Automatically scans specified directories for z-index values in JavaScript, TypeScript, CSS, and SCSS files.
* Interactive Visualization: Presents z-index data in an easy-to-understand chart format.
* Flexible Grouping: Group z-index occurrences by file or individual components.
* Advanced Filtering: Filter results by z-index range, file types, and custom search terms.
* Sorting Options: Sort data by z-index value or file path in ascending or descending order.
* Component Viewer: Examine the context of z-index usage with syntax-highlighted code snippets.
* WebSocket Integration: Provides real-time updates as files change in the monitored directory.
* Responsive Design: Fully responsive interface that works on desktop and mobile devices.

**Technology Stack**
-------------------

* Backend: Rust with Actix Web framework
* Frontend: HTML, CSS, and JavaScript
* WebSocket: For real-time communication between client and server
* Visualization: Chart.js for data visualization
* Syntax Highlighting: Prism.js for code snippet highlighting

**Installation**
--------------

1. Ensure you have Rust and Cargo installed on your system. If not, follow the official Rust installation guide.
2. Clone the repository:
```bash
git clone https://github.com/your-username/Z-Index-Mapper.git
```
3. Build the project:
```bash
cd Z-Index-Mapper
cargo build
```
4. Run the server:
```bash
cargo run
```
5. Open a web browser and navigate to http://localhost:8080 to access the Z-Index Mapper interface.

**Usage**
--------

1. Set Directory: Enter the path to your project directory in the "Directory Path" input field and click "Set".
2. Explore Data: Once the directory is set, the tool will scan for z-index values and display them in the interface.
3. Grouping: Use the "Group by" dropdown to switch between grouping by z-index or file.
4. Sorting: Adjust the "Sort by" and "Sort Order" dropdowns to organize the data as needed.
5. Filtering:
	* Use the "Min Z-Index" and "Max Z-Index" inputs to focus on a specific range of z-index values.
	* Enter file extensions in the "File Type" input to filter by specific file types (e.g., "js,tsx").
	* Use the search bar to find specific z-index values, file names, or line numbers.
6. Visualization: Toggle the chart view using the "Toggle Chart" button to see a graphical representation of z-index usage.
7. Component Viewing: Click on any component card to view the surrounding code context in the Component Viewer.
8. Refresh Data: Click the "Refresh Data" button to manually trigger a rescan of the directory.

**Project Structure**
--------------------

* `src/main.rs`: Contains the Rust backend code, including WebSocket handling, file scanning logic, and HTTP server setup.
* `src/index.html`: The single-page frontend application, including HTML structure, CSS styles, and JavaScript for interactivity and data management.
