<section style="position: relative; width: 100vw; min-height: 100vh; height: 1005px; margin: 0; background: linear-gradient(180deg, #000000 74.2%, rgba(0, 0, 0, 0.5) 86.49%, rgba(0, 0, 0, 0) 98%); overflow: hidden; font-family: 'Inter', sans-serif; left: 0; right: 0;">
  <style>
    @import url("https://fonts.googleapis.com/css2?family=Instrument+Serif:ital,wght@0,400;1,400&family=Inter:wght@400;500&display=swap");

    .hero-wrapper {
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: 24px;
      padding-top: 180px;
      z-index: 2;
      position: relative;
    }

.mobile-br {
  display: none;
}

@media (max-width: 700px) {
  .mobile-br {
    display: inline;
  }
}

    .hero-title {
      font-family: "Instrument Serif", serif;
      font-size: 86px;
      line-height: 1;
      color: #fff;
      text-align: center;
      letter-spacing: -2.58px;
      margin: 0;
    }

    .hero-title.italic {
      font-style: italic;
    }

    .hero-subtext {
      font-size: 20px;
      line-height: 28px;
      color: rgba(255, 255, 255, 0.7);
      text-align: center;
      letter-spacing: -0.2px;
      margin-top: 0px;
    }

    .cta-button {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 6px;
      background: #52A2ED;
      box-shadow: inset 0 0 10px rgba(255, 255, 255, 0.65);
      border-radius: 100px;
      padding: 14px 26px;
      font-size: 16px;
      font-weight: 500;
      color: white;
      letter-spacing: -0.32px;
      margin-top: 12px;
    }

    .cta-button img {
      width: 20px;
      height: 20px;
    }

    .blur-backdrop {
      position: absolute;
      width: 950px;
      height: 950px;
      top: -400px;
      left: 50%;
      transform: translateX(-50%);
      background: radial-gradient(50% 50% at 50% 50%, rgba(37, 139, 255, 0.35) 0%, rgba(37, 139, 255, 0.15) 53.15%, rgba(37, 139, 255, 0.08) 100%);
      filter: blur(50px);
      border-radius: 950px;
      z-index: 0;
    }

.phone-img {
  position: absolute;
  width: 473px;
  height: 654px;
  left: calc(50% - 473px / 2);
  top: 120px;
  z-index: 3;
  pointer-events: none;
}

    .phone-img img {
      width: 100%;
      height: 100%;
      display: block;
    }

.gallery {
  position: absolute;
  bottom: 0;
  left: 50%;
  transform: translateX(-50%);
  width: 200vw;
  height: 240px;
  display: flex;
  gap: 24px;
  align-items: center;
  animation: scrollGallery 60s linear infinite;
  background: linear-gradient(to bottom, rgba(0, 0, 0, 0) 0%, #000 55%, #000 100%);
  mask-image: 
    linear-gradient(to right, transparent 0%, black 15%, black 85%, transparent 100%),
    linear-gradient(to bottom, transparent 0%, black 55%, black 100%);
  -webkit-mask-image: 
    linear-gradient(to right, transparent 0%, black 15%, black 85%, transparent 100%),
    linear-gradient(to bottom, transparent 0%, black 55%, black 100%);
  mask-composite: intersect;
  -webkit-mask-composite: destination-in;
  z-index: 1;
}


    .gallery img {
      width: 240px;
      height: 240px;
      border-radius: 25px;
      object-fit: cover;
    }

    @keyframes scrollGallery {
      0% { transform: translateX(0); }
      100% { transform: translateX(-50%); }
    }

    @media (min-width: 1024px) {
      .phone-img {
        top: 490px;
        left: calc(50% - 473px / 2 + 40px);
      }
    }

    @media (max-width: 1023px) {
      .phone-img {
        top: 48vh;
        left: 50%;
        transform: translateX(-50%);
      }
    }

    .main-nav {
      display: flex;
      flex-direction: row;
      justify-content: center;
      align-items: center;
      padding: 7px 26px;
      gap: 26px;
      width: 597px;
      height: 55.88px;
      background: linear-gradient(180deg, rgba(94, 94, 94, 0.4) 0%, rgba(40, 40, 40, 0.4) 100%);
      box-shadow: inset 0px 1px 2px rgba(255, 255, 255, 0.2);
      backdrop-filter: blur(30px);
      border-radius: 100px;
      position: absolute;
      left: 50%;
      top: 40px;
      transform: translateX(-50%);
      z-index: 10;
    }

    .nav-logo img {
      width: 42px;
      height: 41.88px;
      border-radius: 12px;
      display: block;
    }

    .nav-links {
      display: flex;
      flex-direction: row;
      align-items: center;
      gap: 26px;
    }

    .nav-link {
      font-family: "Inter", sans-serif;
      font-style: normal;
      font-weight: 500;
      font-size: 16px;
      line-height: 19px;
      letter-spacing: -0.32px;
      text-transform: capitalize;
      color: #FFFFFF;
      text-decoration: none;
      padding: 0 8px;
    }

    .nav-dot {
      width: 4px;
      height: 4px;
      background: rgba(255, 255, 255, 0.5);
      border-radius: 100px;
      display: inline-block;
    }

    .nav-hamburger {
      display: none;
      background: none;
      border: none;
      flex-direction: column;
      justify-content: center;
      align-items: center;
      height: 32px;
      width: 32px;
      margin-left: auto;
      cursor: pointer;
    }

    .nav-hamburger span {
      display: block;
      width: 24px;
      height: 3px;
      margin: 3px 0;
      background: #fff;
      border-radius: 2px;
    }

@media (max-width: 700px) {
  body, section {
    overflow-x: hidden !important;
  }

  .gallery {
    width: 200vw;
    left: 0;
    transform: none;
  }

  .main-nav {
    width: calc(100vw - 32px);
    height: 60px;
    padding: 12px 16px;
    top: 16px;
    left: 0;
    right: 0;
    transform: none;
    justify-content: space-between;
    border-radius: 32px;
    margin: 0 auto;
    z-index: 1000;
  }

  body, section {
    overflow-x: hidden !important;
  }

  .gallery {
    width: 200vw;
    left: 0;
    transform: none;
  }

  .nav-links, .nav-dot {
    display: none;
  }

  .nav-logo img {
    width: 36px;
    height: 36px;
  }

  .nav-hamburger {
    display: flex;
  }

  .hero-wrapper {
    padding-top: 100px;
    gap: 16px;
  }

  .hero-title {
    font-size: 48px;
    letter-spacing: -1.2px;
  }

  .hero-subtext {
    font-size: 20px;
    line-height: 28px;
  }
}

  </style>

  <nav class="main-nav">
    <a href="#" class="nav-logo"><img src="https://canavita.abl.ad/wp-content/uploads/2025/05/image.webp" alt="Logo" /></a>
    <div class="nav-links">
      <a href="#" class="nav-link">Home</a>
      <span class="nav-dot"></span>
      <a href="#" class="nav-link">Features</a>
      <span class="nav-dot"></span>
      <a href="#" class="nav-link">Gallery</a>
      <span class="nav-dot"></span>
      <a href="#" class="nav-link">Pricing</a>
    </div>
    <button class="nav-hamburger" aria-label="Open menu">
      <span></span>
      <span></span>
      <span></span>
    </button>
  </nav>

  <div class="blur-backdrop"></div>

  <div class="hero-wrapper">
    <h1 class="hero-title">Break Free Without</h1>
    <h1 class="hero-title italic">Vanishing</h1>
    <p class="hero-subtext">The average person spends 4.5 hours a day on social media.<br>
      LightFriend's SMSs and voice interface connects your digital life without the distractions.</p>

    <div class="cta-button">
      Start Using Now
    </div>
  </div>

  <div class="gallery">
    <img src="https://framerusercontent.com/images/KtfZCxElWCVxgRBmdYefVwMBB0.jpg?scale-down-to=512" alt="">
    <img src="https://framerusercontent.com/images/aHsoNTKCgqSSLEuwADCGE1eBk.jpg" alt="">
    <img src="https://framerusercontent.com/images/aETngQZb5HD3dGHusuL265Bu4.jpg" alt="">
    <img src="https://framerusercontent.com/images/exQBhRyujmSH9PDM3AEuJx7hHJo.jpg" alt="">
    <img src="https://framerusercontent.com/images/rCSj7pZYD6Rh3cSKprVJnoNl70.jpg" alt="">
    <img src="https://framerusercontent.com/images/KtfZCxElWCVxgRBmdYefVwMBB0.jpg?scale-down-to=512" alt="">
    <img src="https://framerusercontent.com/images/aHsoNTKCgqSSLEuwADCGE1eBk.jpg" alt="">
    <img src="https://framerusercontent.com/images/aETngQZb5HD3dGHusuL265Bu4.jpg" alt="">
    <img src="https://framerusercontent.com/images/exQBhRyujmSH9PDM3AEuJx7hHJo.jpg" alt="">
    <img src="https://framerusercontent.com/images/rCSj7pZYD6Rh3cSKprVJnoNl70.jpg" alt="">
  </div>

  <div class="phone-img">
    <img src="https://canavita.abl.ad/wp-content/uploads/2025/05/image-2-scaled.png" alt="Phone UI">
  </div>
    <img src="https://framerusercontent.com/images/KtfZCxElWCVxgRBmdYefVwMBB0.jpg?scale-down-to=512" alt="">
    <img src="https://framerusercontent.com/images/aHsoNTKCgqSSLEuwADCGE1eBk.jpg" alt="">
    <img src="https://framerusercontent.com/images/aETngQZb5HD3dGHusuL265Bu4.jpg" alt="">
    <img src="https://framerusercontent.com/images/exQBhRyujmSH9PDM3AEuJx7hHJo.jpg" alt="">
    <img src="https://framerusercontent.com/images/rCSj7pZYD6Rh3cSKprVJnoNl70.jpg" alt="">
    <img src="https://framerusercontent.com/images/KtfZCxElWCVxgRBmdYefVwMBB0.jpg?scale-down-to=512" alt="">
    <img src="https://framerusercontent.com/images/aHsoNTKCgqSSLEuwADCGE1eBk.jpg" alt="">
    <img src="https://framerusercontent.com/images/aETngQZb5HD3dGHusuL265Bu4.jpg" alt="">
    <img src="https://framerusercontent.com/images/exQBhRyujmSH9PDM3AEuJx7hHJo.jpg" alt="">
    <img src="https://framerusercontent.com/images/rCSj7pZYD6Rh3cSKprVJnoNl70.jpg" alt="">
  </div>
</section>
