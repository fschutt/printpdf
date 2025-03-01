import init, {
    Pdf_HtmlToDocument,
    Pdf_BytesToDocument,
    Pdf_PageToSvg,
    Pdf_DocumentToBytes,
    Pdf_ResourcesForPage,
} from './pkg/printpdf.js';

await init(); // Initialize WASM

let pdfDocument = null; // Store the PdfDocument object
let currentTab = 'html-to-pdf';
let currentPageNumber = 1;
let images = {}; // Store uploaded images (filename => base64)
let fonts = {};  // Store uploaded fonts (filename => base64)
let signatureImageBase64 = null;

// HTML Examples
const htmlExamples = {
    'ramen-recipe': `<!DOCTYPE html>
<html title="Japanese Ramen Recipe">
    <head>
        <style>
            body { font-family: "NotoSansJP", sans-serif; padding: 20mm; }
            h1, h2 { color: #D63031; }
            .section { margin-bottom: 15mm; }
            .ingredients { background: #FFF7E0; padding: 10px; border-radius: 5px; }
            .steps { background: #F1F9FF; padding: 10px; border-radius: 5px; }
            .tip { background: #E8FDF5; padding: 5px; border-radius: 3px; margin-top: 10px; }
            img { border-radius: 5px; }
            table { width: 100%; border-collapse: collapse; }
            td, th { border: 1px solid #ddd; padding: 8px; }
            th { background-color: #f2f2f2; }
        </style>
        
        <header exclude-pages="1">
            <div style="text-align: right; color: #888; font-size: 10px;">
                Japanese Cuisine Recipes - Page <page-number/>
            </div>
            <hr style="color: #ddd;">
        </header>
        
        <footer>
            <hr style="color: #ddd;">
            <div style="text-align: center; color: #888; font-size: 10px;">
                © 2025 Japanese Cuisine Recipes Collection
            </div>
        </footer>
    </head>
    <body>
        <div class="section">
            <h1 style="text-align: center; font-size: 24px;">とんこつラーメン</h1>
            <h2 style="text-align: center; font-size: 18px;">Tonkotsu Ramen</h2>
            
            <div style="text-align: center; margin: 20px 0;">
                <img src="ramen.png" style="width: 80%; max-width: 500px; height: auto;"></img>
            </div>
            
            <p style="text-align: center; font-style: italic;">
                とんこつラーメンは、豚骨を長時間煮込んで作るクリーミーで濃厚なスープが特徴の日本の伝統的な麺料理です。
            </p>
            <p style="text-align: center; font-style: italic;">
                Tonkotsu ramen is a traditional Japanese noodle dish featuring a creamy, rich soup made by simmering pork bones for many hours.
            </p>
        </div>
        
        <div class="section ingredients">
            <h2>材料 (4人分) / Ingredients (Serves 4)</h2>
            <table>
                <tr>
                    <th>日本語 / Japanese</th>
                    <th>英語 / English</th>
                    <th>量 / Amount</th>
                </tr>
                <tr>
                    <td>豚骨</td>
                    <td>Pork Bones</td>
                    <td>1.5 kg</td>
                </tr>
                <tr>
                    <td>ラーメン麺</td>
                    <td>Ramen Noodles</td>
                    <td>4 portions</td>
                </tr>
                <tr>
                    <td>チャーシュー</td>
                    <td>Chashu (Braised Pork Belly)</td>
                    <td>200g</td>
                </tr>
                <tr>
                    <td>味玉</td>
                    <td>Ajitama (Marinated Soft-Boiled Egg)</td>
                    <td>4</td>
                </tr>
                <tr>
                    <td>長ねぎ</td>
                    <td>Green Onions</td>
                    <td>2 stalks</td>
                </tr>
                <tr>
                    <td>もやし</td>
                    <td>Bean Sprouts</td>
                    <td>200g</td>
                </tr>
                <tr>
                    <td>にんにく</td>
                    <td>Garlic</td>
                    <td>4 cloves</td>
                </tr>
                <tr>
                    <td>生姜</td>
                    <td>Ginger</td>
                    <td>30g</td>
                </tr>
                <tr>
                    <td>醤油</td>
                    <td>Soy Sauce</td>
                    <td>100ml</td>
                </tr>
                <tr>
                    <td>みりん</td>
                    <td>Mirin</td>
                    <td>50ml</td>
                </tr>
                <tr>
                    <td>料理酒</td>
                    <td>Cooking Sake</td>
                    <td>50ml</td>
                </tr>
            </table>
        </div>
        
        <div class="section steps">
            <h2>作り方 / Instructions</h2>
            <ol>
                <li>
                    <p><strong>豚骨スープを作る / Prepare the tonkotsu broth</strong></p>
                    <p>豚骨を水で洗い、冷水から鍋に入れて強火で沸騰させます。沸騰したら一度ゆで汁を捨て、豚骨を洗います。</p>
                    <p>Wash the pork bones, place them in a pot with cold water, and bring to a boil over high heat. Once boiling, discard the water and wash the bones.</p>
                </li>
                <li>
                    <p><strong>スープを煮込む / Simmer the broth</strong></p>
                    <p>豚骨と新しい水、にんにく、生姜を鍋に入れ、弱火で8〜12時間煮込みます。途中で水を足して適切な量を維持します。</p>
                    <p>Place the bones in the pot with fresh water, garlic, and ginger. Simmer on low heat for 8-12 hours, adding water as needed to maintain the proper level.</p>
                </li>
                <li>
                    <p><strong>タレを作る / Make the tare (seasoning base)</strong></p>
                    <p>醤油、みりん、料理酒を小鍋に入れて沸騰させ、アルコール分を飛ばします。</p>
                    <p>Combine soy sauce, mirin, and cooking sake in a small pot. Bring to a boil to cook off the alcohol.</p>
                </li>
                <li>
                    <p><strong>麺を茹でる / Cook the noodles</strong></p>
                    <p>ラーメン麺を袋の指示に従って茹でます。通常は2〜3分です。</p>
                    <p>Cook the ramen noodles according to package instructions, typically 2-3 minutes.</p>
                </li>
                <li>
                    <p><strong>ラーメンを組み立てる / Assemble the ramen</strong></p>
                    <p>丼にタレを入れ、スープを注ぎ、麺を入れます。その上にチャーシュー、味玉、長ねぎ、もやしをのせて完成です。</p>
                    <p>Place tare in a bowl, pour in the broth, and add the noodles. Top with chashu, ajitama, green onions, and bean sprouts to serve.</p>
                </li>
            </ol>
            
            <div class="tip">
                <p><strong>ヒント / Tip:</strong> 本格的な豚骨スープを作るには、最低でも8時間、できれば12時間煮込むことをお勧めします。豚骨から十分にコラーゲンとうま味を抽出するために必要です。</p>
                <p>For authentic tonkotsu broth, simmer for at least 8 hours, preferably 12 hours. This is necessary to extract sufficient collagen and umami from the pork bones.</p>
            </div>
        </div>
    </body>
</html>`,

    'synthwave-gallery': `<!DOCTYPE html>
<html title="Synthwave Digital Art Gallery">
    <head>
        <style>
            body {
                background: linear-gradient(135deg, #0f0c29, #302b63, #24243e);
                color: #fff;
                font-family: 'Orbitron', sans-serif;
                padding: 10mm;
            }
            h1, h2 {
                color: #ff00cc;
                text-shadow: 0 0 10px #ff00cc, 0 0 20px #ff00cc;
            }
            .gallery {
                display: flex;
                flex-wrap: wrap;
                justify-content: space-around;
            }
            .gallery-item {
                margin: 5mm;
                background: rgba(0, 0, 0, 0.5);
                border-radius: 5px;
                overflow: hidden;
                box-shadow: 0 0 20px rgba(255, 0, 204, 0.5);
                transition: transform 0.3s;
                width: 45%;
            }
            .gallery-item img {
                width: 100%;
                display: block;
            }
            .gallery-caption {
                padding: 10px;
                text-align: center;
                background: rgba(0, 0, 0, 0.7);
            }
            .grid-container {
                display: flex;
                flex-wrap: wrap;
                gap: 10px;
                margin-top: 10mm;
            }
            .grid-item {
                flex: 1;
                min-width: 30%;
                height: 80px;
                background: linear-gradient(45deg, #fc00ff, #00dbde);
                border-radius: 5px;
            }
            .intro {
                border-left: 5px solid #ff00cc;
                padding-left: 10px;
                margin: 20px 0;
            }
        </style>
        
        <header>
            <div style="display: flex; justify-content: space-between; align-items: center;">
                <div style="font-size: 12px; color: #00eeff;">DIGITAL ART COLLECTION</div>
                <div style="font-size: 12px; color: #00eeff;">PAGE <page-number/></div>
            </div>
            <hr style="border-color: #ff00cc; height: 2px; background-color: #ff00cc; border: none;">
        </header>
        
        <footer>
            <hr style="border-color: #00eeff; height: 1px; background-color: #00eeff; border: none;">
            <div style="text-align: center; color: #00eeff; font-size: 10px;">
                © 2025 SYNTHWAVE ARCHIVES
            </div>
        </footer>
    </head>
    <body>
        <h1 style="text-align: center; font-size: 36px; letter-spacing: 5px;">SYNTHWAVE</h1>
        <h2 style="text-align: center; font-size: 24px; letter-spacing: 3px;">DIGITAL ART COLLECTION</h2>
        
        <div class="intro">
            <p>Welcome to the definitive collection of synthwave and retrowave digital art. This gallery showcases the neon-soaked aesthetics of 80s-inspired futurism, featuring stunning works from the most innovative digital artists in the scene.</p>
        </div>
        
        <div class="gallery">
            <div class="gallery-item">
                <img src="neon_city.png" alt="Neon City"></img>
                <div class="gallery-caption">
                    <h3>NEON CITY</h3>
                    <p>Digital artwork featuring a futuristic cityscape with glowing neon lights</p>
                </div>
            </div>
            
            <div class="gallery-item">
                <img src="cyber_sunset.png" alt="Cyber Sunset"></img>
                <div class="gallery-caption">
                    <h3>CYBER SUNSET</h3>
                    <p>Retrowave sun setting over a digital grid landscape</p>
                </div>
            </div>
            
            <div class="gallery-item">
                <img src="digital_highway.png" alt="Digital Highway"></img>
                <div class="gallery-caption">
                    <h3>DIGITAL HIGHWAY</h3>
                    <p>Endless road through a neon-lit digital universe</p>
                </div>
            </div>
            
            <div class="gallery-item">
                <img src="retro_arcade.png" alt="Retro Arcade"></img>
                <div class="gallery-caption">
                    <h3>RETRO ARCADE</h3>
                    <p>80s-inspired gaming environment with classic arcade machines</p>
                </div>
            </div>
        </div>
        
        <h2 style="text-align: center; margin-top: 20mm;">COLOR PALETTES</h2>
        
        <div class="grid-container">
            <div class="grid-item" style="background: linear-gradient(45deg, #fc00ff, #00dbde);"></div>
            <div class="grid-item" style="background: linear-gradient(45deg, #3f5efb, #fc466b);"></div>
            <div class="grid-item" style="background: linear-gradient(45deg, #0072ff, #00c6ff);"></div>
            <div class="grid-item" style="background: linear-gradient(45deg, #f953c6, #b91d73);"></div>
            <div class="grid-item" style="background: linear-gradient(45deg, #7f00ff, #e100ff);"></div>
            <div class="grid-item" style="background: linear-gradient(45deg, #ff0099, #493240);"></div>
        </div>
        
        <div style="margin-top: 15mm; text-align: center;">
            <p>The synthwave aesthetic draws inspiration from 1980s pop culture, combining the visual elements of that era with modern digital art techniques. At its core are vibrant neon colors contrasted against dark backgrounds.</p>
        </div>
        
        <div style="page-break-before: always;">
            <h2 style="text-align: center; margin-top: 10mm;">FEATURED TECHNIQUES</h2>
            
            <div style="display: flex; justify-content: space-between; margin-top: 10mm;">
                <div style="width: 48%;">
                    <img src="wireframe_technique.png" alt="Wireframe Technique" style="width: 100%;"></img>
                    <h3 style="color: #00eeff;">WIREFRAMES</h3>
                    <p>The use of simple wireframe models creates depth while maintaining the retro-digital aesthetic that defines synthwave art.</p>
                </div>
                
                <div style="width: 48%;">
                    <img src="scanline_technique.png" alt="Scanline Technique" style="width: 100%;"></img>
                    <h3 style="color: #00eeff;">SCANLINES</h3>
                    <p>Scanlines invoke the feel of vintage CRT monitors, adding an authentic retro quality to digital compositions.</p>
                </div>
            </div>
            
            <div style="display: flex; justify-content: space-between; margin-top: 10mm;">
                <div style="width: 48%;">
                    <img src="chrome_technique.png" alt="Chrome Technique" style="width: 100%;"></img>
                    <h3 style="color: #ff00cc;">CHROME EFFECTS</h3>
                    <p>Reflective chrome surfaces were a staple of 80s futurism, representing the sleek design aesthetics of the period.</p>
                </div>
                
                <div style="width: 48%;">
                    <img src="grid_technique.png" alt="Grid Technique" style="width: 100%;"></img>
                    <h3 style="color: #ff00cc;">GRID PERSPECTIVES</h3>
                    <p>Infinite grids extending to the horizon create the illusion of digital landscapes that stretch endlessly.</p>
                </div>
            </div>
        </div>
    </body>
</html>`,

    'business-report': `<!DOCTYPE html>
<html title="Q1 2025 Financial Performance Report">
    <head>
        <style>
            body {
                font-family: 'Arial', sans-serif;
                color: #333;
                line-height: 1.5;
                padding: 15mm;
            }
            h1 {
                color: #1a5276;
                border-bottom: 2px solid #1a5276;
                padding-bottom: 5px;
            }
            h2 {
                color: #2874a6;
                margin-top: 15mm;
            }
            h3 {
                color: #3498db;
            }
            .executive-summary {
                background-color: #f8f9fa;
                border-left: 5px solid #2874a6;
                padding: 10px;
                margin: 15px 0;
            }
            table {
                width: 100%;
                border-collapse: collapse;
                margin: 15px 0;
            }
            th {
                background-color: #1a5276;
                color: white;
                font-weight: bold;
                text-align: left;
                padding: 8px;
                border: 1px solid #ddd;
            }
            td {
                padding: 8px;
                border: 1px solid #ddd;
            }
            tr:nth-child(even) {
                background-color: #f2f2f2;
            }
            .highlight {
                background-color: #e8f4f8;
                padding: 5px;
                border-radius: 3px;
            }
            .chart-container {
                text-align: center;
                margin: 20px 0;
            }
            .footer-note {
                font-size: 10px;
                color: #777;
                font-style: italic;
                text-align: center;
                margin-top: 10mm;
            }
            .kpi-cards {
                display: flex;
                justify-content: space-between;
                margin: 20px 0;
            }
            .kpi-card {
                width: 30%;
                padding: 15px;
                border-radius: 5px;
                box-shadow: 0 2px 5px rgba(0,0,0,0.1);
                text-align: center;
            }
            .positive {
                color: #27ae60;
                font-weight: bold;
            }
            .negative {
                color: #c0392b;
                font-weight: bold;
            }
            .neutral {
                color: #f39c12;
                font-weight: bold;
            }
        </style>
        
        <header exclude-pages="1">
            <div style="display: flex; justify-content: space-between; align-items: center;">
                <div>ACME Corporation</div>
                <div>Q1 2025 Financial Report - Page <page-number/></div>
            </div>
            <hr>
        </header>
        
        <footer>
            <hr>
            <div style="display: flex; justify-content: space-between; align-items: center;">
                <div>Confidential & Proprietary</div>
                <div>© 2025 ACME Corporation</div>
            </div>
        </footer>
    </head>
    <body>
        <div style="text-align: center; margin-bottom: 20mm;">
            <h1 style="font-size: 24px; border: none;">ACME CORPORATION</h1>
            <h2 style="font-size: 20px; margin-top: 5px;">Q1 2025 Financial Performance Report</h2>
            <p style="font-style: italic;">Prepared for: Board of Directors and Shareholders</p>
            <p>March 31, 2025</p>
        </div>
        
        <div class="executive-summary">
            <h3>Executive Summary</h3>
            <p>ACME Corporation delivered strong financial performance in Q1 2025, with revenue growth of 12.3% year-over-year, exceeding our forecast of 10%. Operating margins improved by 2.1 percentage points to 18.5%, driven by operational efficiencies and strategic pricing initiatives. Our Technology segment continues to be the primary growth driver, while the Consumer Products division showed signs of recovery after two challenging quarters.</p>
        </div>
        
        <h2>Key Performance Indicators</h2>
        
        <div class="kpi-cards">
            <div class="kpi-card" style="background-color: #e8f8f5;">
                <h3>Revenue</h3>
                <p style="font-size: 24px;">$287.5M</p>
                <p class="positive">+12.3% YoY</p>
            </div>
            
            <div class="kpi-card" style="background-color: #fef9e7;">
                <h3>Operating Margin</h3>
                <p style="font-size: 24px;">18.5%</p>
                <p class="positive">+2.1pts YoY</p>
            </div>
            
            <div class="kpi-card" style="background-color: #ebf5fb;">
                <h3>Net Income</h3>
                <p style="font-size: 24px;">$42.3M</p>
                <p class="positive">+15.7% YoY</p>
            </div>
        </div>
        
        <h2>Financial Performance by Segment</h2>
        
        <table>
            <tr>
                <th>Business Unit</th>
                <th>Revenue ($M)</th>
                <th>YoY Growth</th>
                <th>Operating Margin</th>
                <th>YoY Change</th>
            </tr>
            <tr>
                <td>Technology</td>
                <td>143.2</td>
                <td class="positive">+18.7%</td>
                <td>24.3%</td>
                <td class="positive">+3.2pts</td>
            </tr>
            <tr>
                <td>Manufacturing</td>
                <td>82.5</td>
                <td class="positive">+8.4%</td>
                <td>15.8%</td>
                <td class="positive">+1.5pts</td>
            </tr>
            <tr>
                <td>Consumer Products</td>
                <td>45.3</td>
                <td class="positive">+5.2%</td>
                <td>12.1%</td>
                <td class="positive">+0.8pts</td>
            </tr>
            <tr>
                <td>Services</td>
                <td>16.5</td>
                <td class="neutral">+2.1%</td>
                <td>14.5%</td>
                <td class="negative">-0.5pts</td>
            </tr>
        </table>
        
        <div class="chart-container">
            <img src="revenue_chart.png" alt="Revenue by Segment" style="width: 80%; max-width: 600px;"></img>
            <p style="font-size: 12px; color: #777;">Revenue distribution by business segment, Q1 2025</p>
        </div>
        
        <h2>Financial Statement Highlights</h2>
        
        <h3>Income Statement</h3>
        <table>
            <tr>
                <th>Metric ($M)</th>
                <th>Q1 2025</th>
                <th>Q1 2024</th>
                <th>YoY Change</th>
            </tr>
            <tr>
                <td>Revenue</td>
                <td>287.5</td>
                <td>256.0</td>
                <td class="positive">+12.3%</td>
            </tr>
            <tr>
                <td>Gross Profit</td>
                <td>143.8</td>
                <td>122.9</td>
                <td class="positive">+17.0%</td>
            </tr>
            <tr>
                <td>Operating Income</td>
                <td>53.2</td>
                <td>42.0</td>
                <td class="positive">+26.7%</td>
            </tr>
            <tr>
                <td>Net Income</td>
                <td>42.3</td>
                <td>36.6</td>
                <td class="positive">+15.7%</td>
            </tr>
            <tr>
                <td>EPS (Diluted)</td>
                <td>$2.15</td>
                <td>$1.87</td>
                <td class="positive">+15.0%</td>
            </tr>
        </table>
        
        <h3>Balance Sheet Highlights</h3>
        <table>
            <tr>
                <th>Metric ($M)</th>
                <th>Mar 31, 2025</th>
                <th>Dec 31, 2024</th>
                <th>Change</th>
            </tr>
            <tr>
                <td>Cash & Equivalents</td>
                <td>175.3</td>
                <td>156.8</td>
                <td class="positive">+11.8%</td>
            </tr>
            <tr>
                <td>Total Assets</td>
                <td>842.7</td>
                <td>825.4</td>
                <td class="positive">+2.1%</td>
            </tr>
            <tr>
                <td>Total Debt</td>
                <td>215.0</td>
                <td>230.0</td>
                <td class="positive">-6.5%</td>
            </tr>
            <tr>
                <td>Shareholders' Equity</td>
                <td>498.4</td>
                <td>462.1</td>
                <td class="positive">+7.9%</td>
            </tr>
        </table>
        
        <div style="page-break-before: always;">
            <h2>Market Analysis & Outlook</h2>
            
            <p>The global market environment remains favorable for our core business segments, despite ongoing geopolitical uncertainties and supply chain challenges. Technology spending continues to show resilience, particularly in digital transformation initiatives and cloud migration projects, which aligns with our strategic focus areas.</p>
            
            <div class="highlight">
                <h3>Key Market Trends</h3>
                <ul>
                    <li><strong>AI Integration:</strong> Increasing demand for AI-powered solutions across industries, particularly in process automation and analytics.</li>
                    <li><strong>Sustainability:</strong> Growing customer preference for eco-friendly products and services, creating new opportunities in our Manufacturing and Consumer Products segments.</li>
                    <li><strong>Digital Experience:</strong> Continued investment in enhanced digital experiences, benefiting our Technology and Services divisions.</li>
                </ul>
            </div>
            
            <h3>FY 2025 Guidance</h3>
            
            <table>
                <tr>
                    <th>Metric</th>
                    <th>Previous Guidance</th>
                    <th>Updated Guidance</th>
                </tr>
                <tr>
                    <td>Revenue Growth</td>
                    <td>9-11%</td>
                    <td class="positive">10-12%</td>
                </tr>
                <tr>
                    <td>Operating Margin</td>
                    <td>17.5-18.5%</td>
                    <td class="positive">18.0-19.0%</td>
                </tr>
                <tr>
                    <td>EPS (Diluted)</td>
                    <td>$8.50-$8.75</td>
                    <td class="positive">$8.75-$9.00</td>
                </tr>
                <tr>
                    <td>Free Cash Flow ($M)</td>
                    <td>$130-$140</td>
                    <td class="positive">$140-$150</td>
                </tr>
            </table>
            
            <h2>Strategic Initiatives Update</h2>
            
            <h3>Digital Transformation Program</h3>
            <p>Our enterprise-wide digital transformation initiative is progressing on schedule and within budget. Key achievements in Q1 include:</p>
            <ul>
                <li>Deployment of advanced analytics platform across 75% of business units, exceeding our target of 65%</li>
                <li>Successful migration of core ERP modules to cloud infrastructure, reducing operational costs by approximately $3.2M annually</li>
                <li>Launch of AI-powered customer service platform, improving response times by 35%</li>
            </ul>
            
            <h3>Product Innovation</h3>
            <p>R&D investments continue to yield positive results, with several new product launches planned for Q2 and Q3. Our innovation pipeline remains strong, with 28 active development projects across all business segments.</p>
            
            <div class="chart-container">
                <img src="pipeline_chart.png" alt="Innovation Pipeline" style="width: 80%; max-width: 600px;"></img>
                <p style="font-size: 12px; color: #777;">Innovation pipeline by development stage and business segment</p>
            </div>
            
            <div class="footer-note">
                <p>This report contains forward-looking statements based on current expectations and projections about future events. These statements are subject to risks and uncertainties that could cause actual results to differ materially from those projected.</p>
            </div>
        </div>
    </body>
</html>`
};

// Event listener for example selection
document.getElementById('html-examples').addEventListener('change', (event) => {
    if (event.target.value) {
        htmlEditorPre.textContent = htmlExamples[event.target.value];
        updateLineNumbers(htmlEditorPre, htmlLineNumbersDiv);
        updatePdfFromHtml();
    }
});

const htmlResourcesContainer = document.getElementById('main-resources-container');
const actionTabSelect = document.getElementById('action-tab');
const tabContents = {
    'html-to-pdf': document.getElementById('html-to-pdf-tab'),
    'parse-edit-pdf': document.getElementById('parse-edit-pdf-tab'),
    'sign-pdf': document.getElementById('sign-pdf-tab'),
};
const htmlEditorPre = document.getElementById('html-editor');
const htmlLineNumbersDiv = document.querySelector('#html-to-pdf-tab .line-numbers');
const jsonEditorPre = document.getElementById('json-editor');
const jsonLineNumbersDiv = document.querySelector('#parse-edit-pdf-tab .line-numbers');
const pdfViewerDiv = document.getElementById('pdf-viewer');
const pageNumberInput = document.getElementById('page-number');
const prevPageButton = document.getElementById('prev-page');
const nextPageButton = document.getElementById('next-page');
const savePdfButton = document.getElementById('save-pdf');
const minimapViewDiv = document.getElementById('minimap-view');
const sidebarModeButtons = document.querySelectorAll('.sidebar-modes button');
const sidebarContents = {
    'minimap': document.getElementById('minimap-view'),
    'layers': document.getElementById('layers-view'),
    'bookmarks': document.getElementById('bookmarks-view'),
};

const imageUploadInput = document.getElementById('image-upload');
const fontUploadInput = document.getElementById('font-upload');
const pdfFileUploadInput = document.getElementById('pdf-file-upload');
const signatureImageUploadInput = document.getElementById('signature-image-upload');
const pdfViewerErrorText = document.getElementById('pdf-viewer-error');

// Create resource previews container for parse tab
const parseResourcesContainer = document.createElement('div');
parseResourcesContainer.className = 'resources-container';
document.querySelector('#parse-edit-pdf-tab .controls').appendChild(parseResourcesContainer);

// Add config file upload input
const configFileInput = document.createElement('input');
configFileInput.type = 'file';
configFileInput.id = 'config-file-upload';
configFileInput.accept = '.json';
configFileInput.style.display = 'none';
document.body.appendChild(configFileInput);

document.getElementById('add-image-html').addEventListener('click', () => imageUploadInput.click());
document.getElementById('add-font-html').addEventListener('click', () => fontUploadInput.click());
document.getElementById('add-image-parse').addEventListener('click', () => imageUploadInput.click());
document.getElementById('add-font-parse').addEventListener('click', () => fontUploadInput.click());
document.getElementById('upload-pdf').addEventListener('click', () => pdfFileUploadInput.click());
document.getElementById('save-config').addEventListener('click', saveConfig);
document.getElementById('load-config').addEventListener('click', () => configFileInput.click());

sidebarModeButtons.forEach(button => {
    button.addEventListener('click', () => {
        sidebarModeButtons.forEach(btn => btn.classList.remove('active'));
        button.classList.add('active');
        const mode = button.dataset.mode;
        Object.values(sidebarContents).forEach(content => content.classList.add('hidden'));
        sidebarContents[mode].classList.remove('hidden');
    });
});

actionTabSelect.addEventListener('change', (event) => {
    currentTab = event.target.value;
    Object.values(tabContents).forEach(content => content.classList.add('hidden'));
    tabContents[currentTab].classList.remove('hidden');
    if (currentTab === 'html-to-pdf') {
        updatePdfFromHtml(); // Initial PDF generation on tab switch if needed
    } else if (currentTab === 'parse-edit-pdf') {
        updateResourcePreviews(parseResourcesContainer, true);
    } else if (currentTab === 'sign-pdf') {
        updatePdfViewer(); // Re-render to show signature if present
    }
});

// Function to base64 encode files
const encodeFileToBase64 = (file, keep_mime) => {
    return new Promise((resolve, reject) => {
        const reader = new FileReader();
        if (keep_mime === true) {
            reader.onload = () => resolve(reader.result);
        } else {
            reader.onload = () => resolve(reader.result.split(',')[1]); // Remove data:base64 prefix
        }
        reader.onerror = reject;
        reader.readAsDataURL(file);
    });
};

imageUploadInput.addEventListener('change', async (event) => {
    const file = event.target.files[0];
    if (file) {
        const base64 = await encodeFileToBase64(file);
        images[file.name] = base64;
        event.target.value = ''; // Reset input
        if (currentTab === 'html-to-pdf') {
            updateResourcePreviews(htmlResourcesContainer);
            updatePdfFromHtml();
        } else if (currentTab === 'parse-edit-pdf') {
            updateResourcePreviews(parseResourcesContainer, true);
            updatePdfFromJsonEditor();
        }
    }
});

fontUploadInput.addEventListener('change', async (event) => {
    const file = event.target.files[0];
    if (file) {
        const base64 = await encodeFileToBase64(file);
        fonts[file.name] = base64;
        event.target.value = ''; // Reset input
        if (currentTab === 'html-to-pdf') {
            updateResourcePreviews(htmlResourcesContainer);
            updatePdfFromHtml();
        } else if (currentTab === 'parse-edit-pdf') {
            updateResourcePreviews(parseResourcesContainer, true);
            updatePdfFromJsonEditor();
        }
    }
});

pdfFileUploadInput.addEventListener('change', async (event) => {
    const file = event.target.files[0];
    if (!file) return;

    const arrayBuffer = await file.arrayBuffer();
    const base64Pdf = await bufferToBase64(new Uint8Array(arrayBuffer));

    try {
        const inputParse = { bytes: base64Pdf, options: {} };
        const inputParseJson = JSON.stringify(inputParse);
        const parseResultJson = await Pdf_BytesToDocument(inputParseJson);
        const parseResult = JSON.parse(parseResultJson);

        if (parseResult.status === 0) {
            pdfDocument = parseResult.data.doc; // Changed from 'pdf' to 'doc'
            for (let i = 0; i < parseResult.data.warnings.length; i++) {
                console.warn(parseResult.data.warnings[i]);
            }
            jsonEditorPre.textContent = JSON.stringify(pdfDocument, null, 2); // Display JSON in editor
            updateLineNumbers(jsonEditorPre, jsonLineNumbersDiv);
            updateResourcePreviews(parseResourcesContainer, true);
            updatePdfViewer();
        } else {
            alert2("PDF Parsing Error: " + parseResult.data);
        }
    } catch (error) {
        alert2("Error parsing PDF: " + error);
    } finally {
        event.target.value = ''; // Reset input
    }
});

configFileInput.addEventListener('change', async (event) => {
    const file = event.target.files[0];
    if (!file) return;
    
    try {
        const text = await file.text();
        const config = JSON.parse(text);
        
        // Load saved configuration
        if (config.images) {
            images = config.images;
        }
        if (config.fonts) {
            fonts = config.fonts;
        }
        if (config.html) {
            htmlEditorPre.textContent = config.html;
            updateLineNumbers(htmlEditorPre, htmlLineNumbersDiv);
        }
        
        updateResourcePreviews(htmlResourcesContainer);
        await updatePdfFromHtml();
    } catch (error) {
        alert2("Error loading configuration: " + error);
    } finally {
        event.target.value = ''; // Reset input
    }
});

signatureImageUploadInput.addEventListener('change', async (event) => {
    const file = event.target.files[0];
    if (file) {
        signatureImageBase64 = await encodeFileToBase64(file, true);
        // Update PDF viewer immediately to preview signature
        await updatePdfViewer();
    }
});

// Update resource previews
function updateResourcePreviews(container, isParseTab = false) {
    container.innerHTML = '';
    
    // Create scrollable container
    const scrollContainer = document.createElement('div');
    scrollContainer.className = 'resources-scroll-container';
    
    // Add images section
    if (Object.keys(images).length > 0) {
        const imagesSection = document.createElement('div');
        imagesSection.className = 'resources-section';
        imagesSection.innerHTML = '<h4>Images</h4>';
        const imagesGrid = document.createElement('div');
        imagesGrid.className = 'resources-grid';
        
        Object.entries(images).forEach(([name, base64]) => {
            const resourceItem = createResourceItem(name, `data:image/png;base64,${base64}`, 'image', isParseTab);
            imagesGrid.appendChild(resourceItem);
        });
        
        imagesSection.appendChild(imagesGrid);
        scrollContainer.appendChild(imagesSection);
    }
    
    // Add fonts section
    if (Object.keys(fonts).length > 0) {
        const fontsSection = document.createElement('div');
        fontsSection.className = 'resources-section';
        fontsSection.innerHTML = '<h4>Fonts</h4>';
        const fontsGrid = document.createElement('div');
        fontsGrid.className = 'resources-grid';
        
        Object.entries(fonts).forEach(([name, base64]) => {
            const resourceItem = createResourceItem(name, null, 'font', isParseTab);
            fontsGrid.appendChild(resourceItem);
        });
        
        fontsSection.appendChild(fontsGrid);
        scrollContainer.appendChild(fontsSection);
    }
    
    container.appendChild(scrollContainer);
}

function createResourceItem(name, src, type, isReadOnly = false) {
    const resourceItem = document.createElement('div');
    resourceItem.className = 'resource-item';
    
    // Preview box
    const preview = document.createElement('div');
    preview.className = 'resource-preview';
    
    if (type === 'image' && src) {
        const img = document.createElement('img');
        img.src = src;
        img.alt = name;
        preview.appendChild(img);
    } else if (type === 'font') {
        preview.textContent = 'Aa';
        preview.className += ' font-preview';
    }
    
    // Name with truncation
    const nameEl = document.createElement('div');
    nameEl.className = 'resource-name';
    nameEl.title = name; // Full name on hover
    nameEl.textContent = name.length > 12 ? name.substring(0, 9) + '...' : name;
    
    // Delete button (only for non-readonly)
    if (!isReadOnly) {
        const deleteBtn = document.createElement('button');
        deleteBtn.className = 'resource-delete';
        deleteBtn.innerHTML = '×';
        deleteBtn.title = 'Remove resource';
        deleteBtn.addEventListener('click', async () => {
            if (type === 'image') {
                delete images[name];
            } else if (type === 'font') {
                delete fonts[name];
            }
            
            // Update previews and re-render PDF
            if (currentTab === 'html-to-pdf') {
                updateResourcePreviews(htmlResourcesContainer);
                await updatePdfFromHtml();
            } else if (currentTab === 'parse-edit-pdf') {
                updateResourcePreviews(parseResourcesContainer, true);
                await updatePdfFromJsonEditor();
            }
        });
        resourceItem.appendChild(deleteBtn);
    }
    
    resourceItem.appendChild(preview);
    resourceItem.appendChild(nameEl);
    
    return resourceItem;
}

// Function to update PDF viewer with SVGs from pdfDocument
async function updatePdfViewer() {
    if (!pdfDocument) return;

    pdfViewerDiv.innerHTML = ''; // Clear existing viewer
    minimapViewDiv.innerHTML = ''; // Clear minimap

    for (let i = 0; i < pdfDocument.pages.length; i++) {
        const page = pdfDocument.pages[i];
        try {
            const resourcesInput = JSON.stringify({ page: page });
            const resourcesJson = await Pdf_ResourcesForPage(resourcesInput);
            const resourcesResult = JSON.parse(resourcesJson);

            if (resourcesResult.status !== 0) {
                console.error("Error getting resources for page:", resourcesResult.data);
                continue; // Skip page rendering if resources fail
            }

            let res = pdfDocument.resources; // Use document resources directly
            
            // Apply signature if in "Sign PDF" tab and on correct page
            let modifiedPage = page;
            if (signatureImageBase64 != null && currentTab === 'sign-pdf' && (i + 1) === parseInt(document.getElementById('signature-page').value)) {
                const applied = applySignatureToPage(page, pdfDocument.resources);
                modifiedPage = applied.data;
                res = applied.resources;
                resourcesResult.data.xobjects.push('user-signature-image');
            }

            const svgI = { 
                page: modifiedPage, 
                resources: copyResourcesForPage(res, resourcesResult.data), 
                options: { imageFormats: ["png", "jpeg"] } // Changed from image_formats to imageFormats
            };
            const svgInput = JSON.stringify(svgI);
            const svgJson = await Pdf_PageToSvg(svgInput);
            const svgResult = JSON.parse(svgJson);

            if (svgResult.status === 0) {
                const svgString = svgResult.data.svg;
                pdfViewerDiv.innerHTML += svgString;
                minimapViewDiv.innerHTML += svgString;
                pdfViewerErrorText.innerHTML = "";
            } else {
                console.error("Error rendering page to SVG:", svgResult.data);
                pdfViewerDiv.innerHTML += `<p class="error">Error rendering page ${i + 1}: ${svgResult.data}</p>`;
            }
        } catch (error) {
            console.error("Error processing page:", error);
            pdfViewerDiv.innerHTML += `<p class="error">Error processing page ${i + 1}: ${error}</p>`;
        }
    }
    updatePageNavigation();
}

/// Returns a new PdfResources object, but with only the resources needed for the page
function copyResourcesForPage(resources, resourcesResult) {
    let newResources = {
        fonts: {},
        xobjects: {},
        layers: {},
        extgstates: resources.extgstates,
    };

    for (let i = 0; i < resourcesResult.xobjects.length; i++) {
        const id = resourcesResult.xobjects[i];
        newResources.xobjects[id] = resources.xobjects[id];
    }

    for (let i = 0; i < resourcesResult.layers.length; i++) {
        const id = resourcesResult.layers[i];
        newResources.layers[id] = resources.layers[id];
    }

    for (let i = 0; i < resourcesResult.fonts.length; i++) {
        const id = resourcesResult.fonts[i];
        newResources.fonts[id] = resources.fonts[id];
    }

    return newResources;
}

function applySignatureToPage(page, resources) {
    if (!signatureImageBase64) return { data: page, resources: resources };
    
    const signatureImageId = 'user-signature-image';
    const res2 = {
        ...resources,
        xobjects: {
            ...resources.xobjects,
            'user-signature-image': { type: 'image', data: signatureImageBase64 },
        }
    };
    
    const signatureX = parseFloat(document.getElementById('signature-x').value);
    const signatureY = parseFloat(document.getElementById('signature-y').value);
    const signatureScaleX = parseFloat(document.getElementById('signature-scale-x').value);
    const signatureScaleY = parseFloat(document.getElementById('signature-scale-y').value);

    const newOps = [...page.ops, {
        type: "use-xobject",
        data: {
            id: signatureImageId,
            transform: {
                translateX: signatureX,
                translateY: signatureY,
                scaleX: signatureScaleX,
                scaleY: signatureScaleY,
                rotate: null,
                dpi: null
            }
        }
    }];

    return {
        resources: res2,
        data: { ...page, ops: newOps }
    };
}

// Function to update PDF from HTML editor content
async function updatePdfFromHtml() {
    const htmlContent = htmlEditorPre.textContent;

    // Extract the title, width, height, etc. from HTML content
    // In a real implementation, these would be parsed from the HTML or CSS
    const generationOptions = {
        pageWidth: 210,
        pageHeight: 297,
        imageOptimization: null, // Changed from imageCompression to imageOptimization
        fontEmbedding: true
    };

    const input = {
        title: "PDF Document",
        html: htmlContent,
        images: images,
        fonts: fonts,
        options: generationOptions
    };

    const inputJson = JSON.stringify(input);

    try {
        const resultJson = await Pdf_HtmlToDocument(inputJson);
        const result = JSON.parse(resultJson);

        if (result.status === 0) {
            pdfDocument = result.data.doc; // Changed from result.data to result.data.doc
            await updatePdfViewer();
        } else {
            alert2("PDF Generation Error: " + result.data);
        }
    } catch (error) {
        alert2("Error generating PDF: " + error);
    }
}

function alert2(s) {
    pdfViewerErrorText.innerHTML = "<p>" + s + "</p>";
}

// Save configuration to JSON file
function saveConfig() {
    const config = {
        html: htmlEditorPre.textContent,
        images: images,
        fonts: fonts
    };
    
    const blob = new Blob([JSON.stringify(config, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'pdf-config.json';
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
}

// Event listener for HTML editor changes (throttled)
let htmlEditorTimeout;
htmlEditorPre.addEventListener('input', () => {
    clearTimeout(htmlEditorTimeout);
    htmlEditorTimeout = setTimeout(() => updatePdfFromHtml(), 500); // 500ms delay
    updateLineNumbers(htmlEditorPre, htmlLineNumbersDiv);
});
htmlEditorPre.addEventListener('scroll', () => {
    htmlLineNumbersDiv.scrollTop = htmlEditorPre.scrollTop;
});
htmlEditorPre.addEventListener('keydown', (event) => {
    if (event.key === 'Tab') { // Handle Tab key for indentation
        event.preventDefault();
        document.execCommand('insertText', false, '\t');
    }
});

// Event listener for JSON editor changes (throttled)
let jsonEditorTimeout;
jsonEditorPre.addEventListener('input', () => {
    clearTimeout(jsonEditorTimeout);
    jsonEditorTimeout = setTimeout(() => updatePdfFromJsonEditor(), 1000); // 1 sec delay
    updateLineNumbers(jsonEditorPre, jsonLineNumbersDiv);
});
jsonEditorPre.addEventListener('scroll', () => {
    jsonLineNumbersDiv.scrollTop = jsonEditorPre.scrollTop;
});
jsonEditorPre.addEventListener('keydown', (event) => {
    if (event.key === 'Tab') { // Handle Tab key for indentation in JSON editor
        event.preventDefault();
        document.execCommand('insertText', false, '\t');
    }
});

async function updatePdfFromJsonEditor() {
    try {
        pdfDocument = JSON.parse(jsonEditorPre.textContent);
        await updatePdfViewer();
    } catch (e) {
        alert2("JSON Parse Error: " + e.message);
    }
}

// PDF Viewer Navigation
prevPageButton.addEventListener('click', () => {
    currentPageNumber = Math.max(1, currentPageNumber - 1);
    updatePageNavigation();
});

nextPageButton.addEventListener('click', () => {
    if (pdfDocument) {
        currentPageNumber = Math.min(pdfDocument.pages.length, currentPageNumber + 1);
        updatePageNavigation();
    }
});

pageNumberInput.addEventListener('change', () => {
    currentPageNumber = Math.max(1, Math.min(pdfDocument ? pdfDocument.pages.length : 1, parseInt(pageNumberInput.value) || 1));
    updatePageNavigation();
});

function updatePageNavigation() {
    pageNumberInput.value = currentPageNumber;
    const pages = pdfViewerDiv.querySelectorAll('svg');
    if (pages.length > 0 && currentPageNumber >= 1 && currentPageNumber <= pages.length) {
        pages[currentPageNumber - 1].scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
}

savePdfButton.addEventListener('click', async () => {
    if (!pdfDocument) {
        alert2("No PDF document to save.");
        return;
    }

    try {
        // Apply signature permanently to PDF if in sign mode
        if (currentTab === 'sign-pdf' && signatureImageBase64) {
            const signaturePage = parseInt(document.getElementById('signature-page').value) - 1;
            if (signaturePage >= 0 && signaturePage < pdfDocument.pages.length) {
                const applied = applySignatureToPage(pdfDocument.pages[signaturePage], pdfDocument.resources);
                
                // Update the signature in the document
                pdfDocument.pages[signaturePage] = applied.data;
                pdfDocument.resources = applied.resources;
            }
        }

        const inputBytes = { doc: pdfDocument, options: {} }; // Changed from 'pdf' to 'doc'
        const inputBytesJson = JSON.stringify(inputBytes);
        const bytesResultJson = await Pdf_DocumentToBytes(inputBytesJson);
        const bytesResult = JSON.parse(bytesResultJson);

        if (bytesResult.status === 0) {
            const base64Pdf = bytesResult.data.bytes; // Changed from pdfBase64 to bytes
            const pdfBytes = base64ToUint8Array(base64Pdf);
            downloadPdf(pdfBytes, "document");
        } else {
            alert2("PDF Serialization Error: " + bytesResult.data);
        }
    } catch (error) {
        alert2("Error saving PDF: " + error);
    }
});

function base64ToUint8Array(base64) {
    const binaryString = atob(base64);
    const byteArray = new Uint8Array(binaryString.length);
    for (let i = 0; i < binaryString.length; i++) {
        byteArray[i] = binaryString.charCodeAt(i);
    }
    return byteArray;
}

// note: `buffer` arg can be an ArrayBuffer or a Uint8Array
// await bufferToBase64(new Uint8Array([1,2,3,100,200]))
async function bufferToBase64(buffer) {
    // use a FileReader to generate a base64 data URI:
    const base64url = await new Promise(r => {
      const reader = new FileReader()
      reader.onload = () => r(reader.result)
      reader.readAsDataURL(new Blob([buffer]))
    });
    // remove the `data:...;base64,` part from the start
    return base64url.slice(base64url.indexOf(',') + 1);
}

function downloadPdf(pdfBytes, filename) {
    const blob = new Blob([pdfBytes], { type: 'application/pdf' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename + '.pdf';
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
}

function updateLineNumbers(editorElement, lineNumberDiv) {
    const lines = editorElement.textContent.split('\n').length;
    let numbers = '';
    for (let i = 1; i <= lines; i++) {
        numbers += i + '\n';
    }
    lineNumberDiv.textContent = numbers;
}

// Initial setup with first example:
updateLineNumbers(htmlEditorPre, htmlLineNumbersDiv);
updateLineNumbers(jsonEditorPre, jsonLineNumbersDiv);

// Set the first example in the dropdown and editor
document.getElementById('html-examples').value = 'ramen-recipe';
htmlEditorPre.textContent = htmlExamples['ramen-recipe'];
updateLineNumbers(htmlEditorPre, htmlLineNumbersDiv);

// Generate initial PDF
updatePdfFromHtml();
