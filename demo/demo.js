(function () {
  let tac = null;
  const result = document.getElementById('result');
  const button = document.getElementById('open-captcha');

  function loadCaptcha() {
    result.textContent = '正在加载验证码…';
    const captchaType = document.getElementById('captcha-type').value;
    const config = {
      requestCaptchaDataUrl: '/gen',
      validCaptchaUrl: '/check',
      bindEl: '#captcha-box',
      validSuccess: function (res, captcha, instance) {
        result.textContent = '验证成功';
        instance.destroyWindow();
      },
      validFail: function (res, captcha, instance) {
        const code = res && res.code;
        result.textContent = code === 50001
          ? '轨迹校验失败：请拖动至少 300ms，移动过程中稍作上下微调，避免过快或突然跳动。'
          : (res && res.msg ? res.msg : '验证失败，请重试');
        instance.reloadCaptcha();
      }
    };
    try {
      if (typeof window.TAC !== 'function') throw new Error('TAC SDK 未加载，请检查 /static/tac.min.js');
      const requestChain = {
        preRequest: function (type, requestParam) {
          if (type === 'requestCaptchaData') {
          requestParam.data = {
            type: captchaType,
            obfuscate: ['SLIDER', 'CONCAT'].includes(captchaType) && document.getElementById('captcha-obfuscate').checked
          };
        }
          return true;
        },
        postRequest: function (type, requestParam, response) {
          if (type !== 'requestCaptchaData' || captchaType !== 'CONCAT' || !response?.data) return true;
          const image = response.data;
          const ratio = image.backgroundImageHeight / image.backgroundImageWidth;
          const observer = new MutationObserver(function () {
            const content = document.querySelector('#captcha-web-sdk .content');
            const overlay = document.querySelector('#captcha-web-sdk .captcha-web-sdk-slider-concat-img-div');
            if (!content || !overlay || !content.clientWidth) return;
            overlay.style.setProperty('background-size', `100% ${content.clientWidth * ratio}px`, 'important');
            observer.disconnect();
          });
          observer.observe(document.querySelector('#captcha-box'), { childList: true, subtree: true });
          requestAnimationFrame(function () {
            const content = document.querySelector('#captcha-web-sdk .content');
            const overlay = document.querySelector('#captcha-web-sdk .captcha-web-sdk-slider-concat-img-div');
            if (content && overlay && content.clientWidth) {
              overlay.style.setProperty('background-size', `100% ${content.clientWidth * ratio}px`, 'important');
              observer.disconnect();
            }
          });
          return true;
        }
      };
      tac = new TAC(config);
      tac.config.insertRequestChain(0, requestChain);
      tac.init();
      setTimeout(function () {
        const disabled = document.querySelector('#captcha-web-sdk.captcha-web-sdk-disable');
        if (disabled) result.textContent = '验证码加载失败，请检查 /gen 接口及浏览器控制台。';
        else if (document.querySelector('#captcha-web-sdk')) result.textContent = '验证码已加载，请完成验证。';
      }, 1200);
    } catch (error) {
      result.textContent = '验证码启动失败：' + error.message;
      console.error(error);
    }
  }

  button.addEventListener('click', loadCaptcha);
  document.getElementById('captcha-type').addEventListener('change', function () {
    const supportsObfuscation = this.value === 'SLIDER' || this.value === 'CONCAT';
    document.getElementById('captcha-obfuscate').disabled = !supportsObfuscation;
    document.getElementById('obfuscate-option').style.opacity = supportsObfuscation ? '1' : '0.45';
  });
  // Deliberately do not initialize TAC on page load; user opens it with the button.
})();
