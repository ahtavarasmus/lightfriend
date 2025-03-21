
.legal-content {
    min-height: 100vh;
    background: #1a1a1a;
    padding: 4rem 2rem;
    text-align: center;
    color: #fff;
    display: flex;
    flex-direction: column;
    align-items: center;
}

.legal-content > div {
    background: rgba(30, 30, 30, 0.7);
    border: 1px solid rgba(30, 144, 255, 0.1);
    border-radius: 16px;
    padding: 3rem;
    max-width: 800px;
    margin: 0 auto;
    backdrop-filter: blur(10px);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
}

.legal-content h1 {
    font-size: 2rem;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    margin-bottom: 2.5rem;
}

.legal-content section {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 12px;
    width: 100%;
    max-width: 600px;
    padding: 2rem;
    margin: 1.5rem auto;
}

.legal-content h2 {
    color: #7EB2FF;
    font-size: 1.5rem;
    margin-bottom: 1.5rem;
}

.legal-content h3 {
    color: #7EB2FF;
    font-size: 1.1rem;
    margin: 1rem 0 0.5rem 0;
}

.legal-content p, .legal-content li {
    color: #999;
    line-height: 1.6;
    margin-bottom: 1rem;
}

.legal-content ul {
    list-style-type: none;
    padding-left: 1.5rem;
}

.legal-content li {
    position: relative;
    margin-bottom: 0.5rem;
}

.legal-content li:before {
    content: "•";
    color: #1E90FF;
    position: absolute;
    left: -1.5rem;
}

.legal-links {
    margin-top: 2rem;
    text-align: center;
}

.legal-links a {
    color: #1E90FF;
    text-decoration: none;
    transition: color 0.3s ease;
}

.legal-links a:hover {
    color: #7EB2FF;
}

