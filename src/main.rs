#![no_std]
#![no_main]

extern crate cortex_m;
extern crate cortex_m_rt as rt;
extern crate panic_probe;
extern crate rtic;
extern crate stm32g0xx_hal as hal;

mod lantern;

use crate::lantern::*;
use hal::prelude::*;
use hal::timer::*;
use hal::{gpio::*, watchdog::IndependedWatchdog};
use hal::{rcc, spi, stm32};
use infrared::{protocols::Nec, PeriodicReceiver};
use smart_leds::SmartLedsWrite;
use ws2812_spi::Ws2812;

pub use defmt_rtt as _;

type IrPin = gpioa::PA11<Input<Floating>>;
type SampleTimer = Timer<stm32::TIM1>;
type AnimationTimer = Timer<stm32::TIM16>;
type SpiLink = spi::Spi<stm32::SPI1, (spi::NoSck, spi::NoMiso, gpioa::PA12<DefaultMode>)>;

#[rtic::app(device = hal::stm32, peripherals = true)]
mod app {
    use super::*;

    #[shared]
    struct Shared {
        lantern: Lantern,
    }

    #[local]
    struct Local {
        link: Ws2812<SpiLink>,
        ir: PeriodicReceiver<Nec, IrPin>,
        animation_timer: AnimationTimer,
        sample_timer: SampleTimer,
        watchdog: IndependedWatchdog,
    }

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let pll_cfg = rcc::PllConfig::with_hsi(4, 24, 2);
        let rcc_cfg = rcc::Config::pll().pll_cfg(pll_cfg);
        let mut rcc = ctx.device.RCC.freeze(rcc_cfg);

        let port_a = ctx.device.GPIOA.split(&mut rcc);
        let spi = ctx.device.SPI1.spi(
            (spi::NoSck, spi::NoMiso, port_a.pa12),
            spi::MODE_0,
            3.MHz(),
            &mut rcc,
        );

        let mut sample_timer = ctx.device.TIM1.timer(&mut rcc);
        sample_timer.start(50.micros());
        sample_timer.listen();

        let mut animation_timer = ctx.device.TIM16.timer(&mut rcc);
        animation_timer.start(80.millis());
        animation_timer.listen();

        let ir = PeriodicReceiver::new(port_a.pa11.into_floating_input(), 20_000);
        let link = Ws2812::new(spi);
        let lantern = Lantern::new();

        let mut watchdog = ctx.device.IWDG.constrain();
        watchdog.start(200.millis());
        defmt::info!("Init completed");

        (
            Shared { lantern },
            Local {
                animation_timer,
                ir,
                link,
                sample_timer,
                watchdog,
            },
            init::Monotonics(),
        )
    }

    #[task(binds = TIM1_BRK_UP_TRG_COM, local = [ir, sample_timer], shared = [lantern])]
    fn sample_timer_tick(ctx: sample_timer_tick::Context) {
        match ctx.local.ir.poll() {
            Ok(Some(cmd)) if cmd.addr == 7 => {
                let mut lantern = ctx.shared.lantern;
                lantern.lock(|lantern| lantern.command(cmd.cmd));
            }
            _ => {}
        }
        ctx.local.sample_timer.clear_irq();
    }

    #[task(binds = TIM16, local = [link, animation_timer, watchdog], shared = [lantern])]
    fn animation_timer_tick(ctx: animation_timer_tick::Context) {
        let mut lantern = ctx.shared.lantern;

        let animation = lantern.lock(|lantern| lantern.animate());
        ctx.local.link.write(animation.into_iter()).ok();
        ctx.local.watchdog.feed();
        ctx.local.animation_timer.clear_irq();
    }
}
