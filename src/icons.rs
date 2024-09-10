use yew::{html, Html};

const SVG_FILL: &str = "currentColor";
const SVG_ROLE: &str = "img";
const SVG_XMLNS: &str = "http://www.w3.org/2000/svg";

// SVG of the icon that looks like >
pub fn chevron() -> Html {
    html! {
        <svg
            role={SVG_ROLE}
            xmlns={SVG_XMLNS}
	        viewBox="0 0 185.343 185.343"
        >
            <path
                fill={SVG_FILL}
                d="M51.707,185.343c-2.741,0-5.493-1.044-7.593-3.149c-4.194-4.194-4.194-10.981,0-15.175
                l74.352-74.347L44.114,18.32c-4.194-4.194-4.194-10.987,0-15.175c4.194-4.194,10.987-4.194,15.18,0l81.934,81.934
                c4.194,4.194,4.194,10.987,0,15.175l-81.934,81.939C57.201,184.293,54.454,185.343,51.707,185.343z"
            />
        </svg>
    }
}

// SVG of a house icon.
pub fn home() -> Html {
    html! {
        <svg
            role={SVG_ROLE}
            xmlns={SVG_XMLNS}
            viewBox="0 0 16 16"
        >
            <path
                fill={SVG_FILL}
                d="M15.45,7L14,5.551V2c0-0.55-0.45-1-1-1h-1c-0.55,0-1,0.45-1,1v0.553L9,0.555C8.727,0.297,8.477,0,8,0S7.273,0.297,7,0.555
                L0.55,7C0.238,7.325,0,7.562,0,8c0,0.563,0.432,1,1,1h1v6c0,0.55,0.45,1,1,1h3v-5c0-0.55,0.45-1,1-1h2c0.55,0,1,0.45,1,1v5h3
                c0.55,0,1-0.45,1-1V9h1c0.568,0,1-0.437,1-1C16,7.562,15.762,7.325,15.45,7z"
            />
        </svg>
    }
}