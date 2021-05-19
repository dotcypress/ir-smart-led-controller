#![no_std]
#![no_main]
#![deny(warnings)]

extern crate cortex_m;
extern crate cortex_m_rt as rt;
extern crate panic_halt;
extern crate rtic;
extern crate stm32g0xx_hal as hal;

mod strip;

use crate::strip::*;
use hal::gpio::*;
use hal::prelude::*;
use hal::time::*;
use hal::timer::*;
use hal::{rcc, spi, stm32};
use infrared::{protocols::Nec, PeriodicReceiver};
use smart_leds::SmartLedsWrite;
use ws2812_spi::Ws2812;

pub use defmt_rtt as _;

pub const STRIP_SIZE: usize = 248;
pub const STRIP_FPS: Hertz = Hertz(24);
pub const IR_ADDRESS: u8 = 0;
pub const IR_SAMPLERATE: Hertz = Hertz(20_000);

pub type IrPin = gpioa::PA11<Input<Floating>>;
pub type SampleTimer = Timer<stm32::TIM14>;
pub type AnimationTimer = Timer<stm32::TIM16>;
pub type SpiLink = spi::Spi<stm32::SPI1, (spi::NoSck, spi::NoMiso, gpioa::PA12<DefaultMode>)>;

#[rtic::app(device = hal::stm32, peripherals = true)]
const APP: () = {
    struct Resources {
        animation_timer: AnimationTimer,
        sample_timer: SampleTimer,
        ir: PeriodicReceiver<Nec, IrPin>,
        link: Ws2812<SpiLink>,
        strip: Strip,
    }

    #[init]
    fn init(ctx: init::Context) -> init::LateResources {
        let pll_cfg = rcc::PllConfig::with_hsi(4, 24, 2);
        let rcc_cfg = rcc::Config::pll().pll_cfg(pll_cfg);
        let mut rcc = ctx.device.RCC.freeze(rcc_cfg);

        let port_a = ctx.device.GPIOA.split(&mut rcc);
        let spi = ctx.device.SPI1.spi(
            (spi::NoSck, spi::NoMiso, port_a.pa12),
            spi::MODE_0,
            3.mhz(),
            &mut rcc,
        );

        let mut sample_timer = ctx.device.TIM14.timer(&mut rcc);
        sample_timer.start(IR_SAMPLERATE);
        sample_timer.listen();

        let mut animation_timer = ctx.device.TIM16.timer(&mut rcc);
        animation_timer.start(STRIP_FPS);
        animation_timer.listen();

        defmt::info!("Init completed");
        init::LateResources {
            animation_timer,
            sample_timer,
            strip: Strip::new(),
            link: Ws2812::new(spi),
            ir: PeriodicReceiver::new(port_a.pa11.into_floating_input(), IR_SAMPLERATE.0),
        }
    }

    #[task(binds = TIM14, resources = [ir, sample_timer, strip])]
    fn sample_timer_tick(ctx: sample_timer_tick::Context) {
        match ctx.resources.ir.poll() {
            Ok(Some(cmd)) if cmd.addr == IR_ADDRESS => {
                defmt::trace!("IR Command: {:x}", cmd.cmd);
                ctx.resources.strip.handle_command(cmd.cmd);
            }
            _ => {}
        }
        ctx.resources.sample_timer.clear_irq();
    }

    #[task(binds = TIM16, resources = [animation_timer, strip])]
    fn animation_timer_tick(ctx: animation_timer_tick::Context) {
        ctx.resources.strip.handle_frame();
        ctx.resources.animation_timer.clear_irq();
    }

    #[idle(resources = [strip, link])]
    fn idle(mut ctx: idle::Context) -> ! {
        loop {
            let animation = ctx.resources.strip.lock(|strip| strip.animate());
            ctx.resources.link.write(animation).ok();
        }
    }
};
